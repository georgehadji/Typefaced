//! [`Document`]: one open document, synchronous. It owns the current `Arc<DocState>`, the
//! undo/redo [`History`] and the open transaction (sandboxes come with Step 7). The actor
//! (Step 8) wraps it in a thread.
//!
//! Every change of the state gets the next [`Revision`]: a command, an undo, a redo and a
//! rollback. A command that changes nothing records nothing. A failed command leaves the
//! state (pointer-equal) and the history as they were.

use std::collections::BTreeSet;
use std::sync::Arc;

use tf_commands::{Command, CommandError, ErrorCode};
use tf_model::Font;

use crate::ctx::{ExecCtx, Origin, Revision, TxnId};
use crate::history::{CoalesceKey, DEFAULT_CAP, History, Step};
use crate::reduce::{self, Event, Touched};

/// The state of a document at one revision. Cloning copies a few pointers.
#[derive(Clone, Debug, PartialEq)]
pub struct DocState {
    pub font: Font,
    pub revision: Revision,
    /// The most glyph-table slots any state of this document has had. Undo can restore a
    /// font with fewer slots; `glyph.create` reserves up to this mark first, so a glyph ID
    /// handed out once (to a client, a sandbox, a redo step) is never handed out again.
    pub glyph_slots: usize,
}

impl DocState {
    /// `font` at `revision`, its own slot count as the mark.
    pub fn new(font: Font, revision: Revision) -> Self {
        let glyph_slots = font.glyphs().slot_count();
        Self {
            font,
            revision,
            glyph_slots,
        }
    }
}

/// What a successful command, undo, redo, commit or rollback did.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    /// The revision after it (unchanged when nothing changed).
    pub revision: Revision,
    pub events: Vec<Event>,
    /// The entities that changed; empty when nothing did.
    pub touched: BTreeSet<Touched>,
}

/// An open transaction: its commands become one undo step.
#[derive(Clone, Debug)]
struct Transaction {
    id: TxnId,
    label: String,
    before: Arc<DocState>,
    touched: BTreeSet<Touched>,
}

/// One open document.
#[derive(Debug)]
pub struct Document {
    state: Arc<DocState>,
    history: History,
    txn: Option<Transaction>,
    next_txn: u32,
}

// The actor (Step 8) moves the document into its thread.
const _: () = {
    const fn send<T: Send>() {}
    send::<Document>();
};

impl Document {
    /// A document on `font` at revision 0, with the default history cap.
    pub fn new(font: Font) -> Self {
        Self::with_history_cap(font, DEFAULT_CAP)
    }

    /// A document that keeps at most `cap` undo steps.
    pub fn with_history_cap(font: Font, cap: usize) -> Self {
        Self {
            state: Arc::new(DocState::new(font, Revision(0))),
            history: History::new(cap),
            txn: None,
            next_txn: 0,
        }
    }

    /// The current state. Readers keep the `Arc`; the document never changes it in place.
    pub fn state(&self) -> &Arc<DocState> {
        &self.state
    }

    /// The undo/redo history.
    pub fn history(&self) -> &History {
        &self.history
    }

    /// The open transaction, if any.
    pub fn transaction(&self) -> Option<TxnId> {
        self.txn.as_ref().map(|txn| txn.id)
    }

    /// Runs one command. `doc.undo` and `doc.redo` go through the history; every other
    /// command through its reducer.
    pub fn execute(&mut self, ctx: &ExecCtx, command: &Command) -> Result<Outcome, CommandError> {
        self.check(ctx)?;
        match command {
            Command::DocUndo(_) => self.undo(),
            Command::DocRedo(_) => self.redo(),
            _ => self.apply(ctx, command),
        }
    }

    /// Opens a transaction: the commands that name it become one undo step labelled
    /// `label`. Transactions do not nest (`E_TRANSACTION_OPEN`).
    pub fn begin(&mut self, label: &str) -> Result<TxnId, CommandError> {
        if let Some(open) = &self.txn {
            return Err(transaction_open(open.id));
        }
        // The commits before a transaction never coalesce with the ones after it.
        self.history.end_run();
        let id = TxnId(self.next_txn);
        self.next_txn = self.next_txn.wrapping_add(1);
        self.txn = Some(Transaction {
            id,
            label: label.to_owned(),
            before: Arc::clone(&self.state),
            touched: BTreeSet::new(),
        });
        Ok(id)
    }

    /// Closes the transaction and records its changes as one undo step (none when it
    /// changed nothing).
    pub fn commit(&mut self, id: TxnId) -> Result<Outcome, CommandError> {
        let txn = self.take_transaction(id)?;
        if !txn.touched.is_empty() {
            self.history
                .record(txn.before, txn.label, txn.touched.clone(), None);
        }
        Ok(self.outcome(Vec::new(), txn.touched))
    }

    /// Closes the transaction and goes back to the state at `begin`.
    pub fn rollback(&mut self, id: TxnId) -> Result<Outcome, CommandError> {
        let txn = self.take_transaction(id)?;
        if !txn.touched.is_empty() {
            self.set_font(txn.before.font.clone());
        }
        Ok(self.outcome(Vec::new(), txn.touched))
    }

    fn take_transaction(&mut self, id: TxnId) -> Result<Transaction, CommandError> {
        self.txn
            .take_if(|txn| txn.id == id)
            .ok_or_else(|| no_transaction(id))
    }

    /// The checks every command passes first: sandbox, base revision, transaction.
    fn check(&self, ctx: &ExecCtx) -> Result<(), CommandError> {
        if let Some(sandbox) = ctx.sandbox {
            return Err(CommandError::new(
                ErrorCode::SandboxNotFound,
                format!("no sandbox {}", sandbox.0),
            ));
        }
        if let Some(base) = ctx.base
            && base != self.state.revision
        {
            return Err(CommandError::new(
                ErrorCode::StaleBase,
                format!(
                    "the command was made against revision {}; the document is at {}",
                    base.0, self.state.revision.0
                ),
            ));
        }
        match (&self.txn, ctx.txn) {
            (None, None) => Ok(()),
            (Some(open), Some(id)) if open.id == id => Ok(()),
            (Some(open), None) => Err(transaction_open(open.id)),
            (_, Some(id)) => Err(no_transaction(id)),
        }
    }

    fn apply(&mut self, ctx: &ExecCtx, command: &Command) -> Result<Outcome, CommandError> {
        let reduced = reduce::reduce(&self.state, command)?;
        if reduced.touched.is_empty() {
            return Ok(self.outcome(reduced.events, BTreeSet::new()));
        }
        let before = std::mem::replace(&mut self.state, Arc::new(reduced.state));
        match &mut self.txn {
            Some(txn) => txn.touched.extend(reduced.touched.iter().cloned()),
            None => {
                let coalesce = (ctx.origin == Origin::User)
                    .then(|| CoalesceKey::of(command).map(|key| (key, ctx.at_ms)))
                    .flatten();
                let label = tf_commands::registry::command(command.id()).title;
                self.history
                    .record(before, label.to_owned(), reduced.touched.clone(), coalesce);
            }
        }
        Ok(self.outcome(reduced.events, reduced.touched))
    }

    fn undo(&mut self) -> Result<Outcome, CommandError> {
        if let Some(open) = &self.txn {
            return Err(transaction_open(open.id));
        }
        let step = self
            .history
            .undo(Arc::clone(&self.state))
            .ok_or_else(|| CommandError::new(ErrorCode::NothingToUndo, "nothing to undo"))?;
        Ok(self.restore(step))
    }

    fn redo(&mut self) -> Result<Outcome, CommandError> {
        if let Some(open) = &self.txn {
            return Err(transaction_open(open.id));
        }
        let step = self
            .history
            .redo(Arc::clone(&self.state))
            .ok_or_else(|| CommandError::new(ErrorCode::NothingToRedo, "nothing to redo"))?;
        Ok(self.restore(step))
    }

    /// Goes to the font of `step` at the next revision.
    fn restore(&mut self, step: Step) -> Outcome {
        self.set_font(step.state.font.clone());
        self.outcome(Vec::new(), step.touched)
    }

    fn set_font(&mut self, font: Font) {
        // Undo, redo and rollback keep the slot mark of the newest state.
        self.state = Arc::new(DocState {
            font,
            revision: self.state.revision.next(),
            glyph_slots: self.state.glyph_slots,
        });
    }

    fn outcome(&self, events: Vec<Event>, touched: BTreeSet<Touched>) -> Outcome {
        Outcome {
            revision: self.state.revision,
            events,
            touched,
        }
    }
}

fn transaction_open(id: TxnId) -> CommandError {
    CommandError::new(
        ErrorCode::TransactionOpen,
        format!(
            "transaction {} is open: name it, or commit or roll it back",
            id.0
        ),
    )
}

fn no_transaction(id: TxnId) -> CommandError {
    CommandError::new(
        ErrorCode::NoTransaction,
        format!("transaction {} is not open", id.0),
    )
}

#[cfg(test)]
mod tests {
    use tf_commands::command::{
        DocRedo, DocUndo, GlyphCreate, GlyphDelete, GlyphId, GlyphSetWidth, LayerId,
    };
    use tf_commands::query::SandboxId;

    use super::*;
    use crate::reduce::fixture;

    fn user(at_ms: u64) -> ExecCtx {
        ExecCtx::new(Origin::User, at_ms)
    }

    fn width(glyph: u32, width: f64) -> Command {
        Command::GlyphSetWidth(GlyphSetWidth {
            id: GlyphId(glyph),
            layer: LayerId(0),
            width,
        })
    }

    fn create(name: &str) -> Command {
        Command::GlyphCreate(GlyphCreate {
            name: name.into(),
            codepoints: vec![],
            width: 0.0,
        })
    }

    const UNDO: Command = Command::DocUndo(DocUndo {});
    const REDO: Command = Command::DocRedo(DocRedo {});

    fn code(result: Result<Outcome, CommandError>) -> ErrorCode {
        result.unwrap_err().code
    }

    #[test]
    fn commands_undo_and_redo_move_the_revision_forward() {
        let mut doc = Document::new(fixture::font());
        let initial = doc.state().font.clone();
        let outcome = doc.execute(&user(0), &width(0, 700.0)).unwrap();
        assert_eq!(outcome.revision, Revision(1));
        assert_eq!(doc.history().undo_label(), Some("Set width"));
        let edited = doc.state().font.clone();

        let undone = doc.execute(&user(1), &UNDO).unwrap();
        assert_eq!(undone.revision, Revision(2));
        assert_eq!(undone.touched, outcome.touched);
        assert_eq!(doc.state().font, initial);
        let redone = doc.execute(&user(2), &REDO).unwrap();
        assert_eq!(redone.revision, Revision(3));
        assert_eq!(doc.state().font, edited);

        assert_eq!(code(doc.execute(&user(3), &REDO)), ErrorCode::NothingToRedo);
        doc.execute(&user(3), &UNDO).unwrap();
        assert_eq!(code(doc.execute(&user(4), &UNDO)), ErrorCode::NothingToUndo);
    }

    #[test]
    fn errors_and_no_ops_change_nothing() {
        let mut doc = Document::new(fixture::font());
        doc.execute(&user(0), &width(0, 700.0)).unwrap();
        doc.execute(&user(1000), &UNDO).unwrap();
        let before = Arc::clone(doc.state());

        assert_eq!(
            code(doc.execute(&user(2000), &create("A"))),
            ErrorCode::NameInUse
        );
        // The same width as now: a no-op keeps the revision and the redo step.
        let same = doc.execute(&user(2000), &width(0, 600.0)).unwrap();
        assert_eq!(same.revision, before.revision);
        assert!(same.touched.is_empty());
        assert!(Arc::ptr_eq(doc.state(), &before));
        assert_eq!((doc.history().undo_len(), doc.history().redo_len()), (0, 1));
    }

    #[test]
    fn the_context_is_checked_first() {
        let mut doc = Document::new(fixture::font());
        let mut ctx = user(0);
        ctx.sandbox = Some(SandboxId(1));
        assert_eq!(
            code(doc.execute(&ctx, &width(0, 1.0))),
            ErrorCode::SandboxNotFound
        );

        let mut ctx = user(0);
        ctx.base = Some(Revision(0));
        doc.execute(&ctx, &width(0, 1.0)).unwrap();
        // Now at revision 1: base 0 is stale, for undo too.
        assert_eq!(
            code(doc.execute(&ctx, &width(0, 2.0))),
            ErrorCode::StaleBase
        );
        assert_eq!(code(doc.execute(&ctx, &UNDO)), ErrorCode::StaleBase);
        ctx.base = Some(Revision(1));
        doc.execute(&ctx, &UNDO).unwrap();
        assert_eq!(doc.state().revision, Revision(2));
    }

    #[test]
    fn a_transaction_is_one_undo_step() {
        let mut doc = Document::new(fixture::font());
        let initial = doc.state().font.clone();
        let txn = doc.begin("Space and width").unwrap();
        assert_eq!(doc.transaction(), Some(txn));
        assert_eq!(
            doc.begin("nested").unwrap_err().code,
            ErrorCode::TransactionOpen
        );

        let mut ctx = user(0);
        // While it is open, a command must name it.
        assert_eq!(
            code(doc.execute(&ctx, &width(0, 1.0))),
            ErrorCode::TransactionOpen
        );
        ctx.txn = Some(txn);
        doc.execute(&ctx, &width(0, 1.0)).unwrap();
        doc.execute(&ctx, &create("B")).unwrap();
        assert_eq!(code(doc.execute(&ctx, &UNDO)), ErrorCode::TransactionOpen);
        assert_eq!(code(doc.execute(&ctx, &REDO)), ErrorCode::TransactionOpen);
        assert_eq!(doc.history().undo_len(), 0);

        let outcome = doc.commit(txn).unwrap();
        assert_eq!(outcome.revision, Revision(2));
        assert_eq!(outcome.touched.len(), 2);
        assert_eq!(doc.history().undo_label(), Some("Space and width"));
        assert_eq!(doc.transaction(), None);
        // The closed transaction cannot be named any more.
        assert_eq!(
            code(doc.execute(&ctx, &width(0, 2.0))),
            ErrorCode::NoTransaction
        );
        assert_eq!(code(doc.commit(txn)), ErrorCode::NoTransaction);
        assert_eq!(code(doc.rollback(txn)), ErrorCode::NoTransaction);

        doc.execute(&user(0), &UNDO).unwrap();
        assert_eq!(doc.state().font, initial);
    }

    #[test]
    fn rollback_restores_the_begin_state() {
        let mut doc = Document::new(fixture::font());
        let initial = doc.state().font.clone();
        let txn = doc.begin("t").unwrap();
        let mut ctx = user(0);
        ctx.txn = Some(txn);
        doc.execute(&ctx, &width(0, 1.0)).unwrap();
        let wrong = TxnId(txn.0 + 1);
        assert_eq!(code(doc.rollback(wrong)), ErrorCode::NoTransaction);
        let outcome = doc.rollback(txn).unwrap();
        assert_eq!(outcome.revision, Revision(2));
        assert_eq!(doc.state().font, initial);
        assert_eq!(doc.history().undo_len(), 0);

        // An empty transaction records nothing and keeps the state.
        let before = Arc::clone(doc.state());
        let txn = doc.begin("empty").unwrap();
        assert_ne!(txn, TxnId(0));
        assert!(doc.commit(txn).unwrap().touched.is_empty());
        let txn = doc.begin("empty").unwrap();
        doc.rollback(txn).unwrap();
        assert!(Arc::ptr_eq(doc.state(), &before));
        assert_eq!(doc.history().undo_len(), 0);
    }

    #[test]
    fn only_user_drags_coalesce() {
        let mut doc = Document::new(fixture::font());
        doc.execute(&user(0), &width(0, 1.0)).unwrap();
        doc.execute(&user(500), &width(0, 2.0)).unwrap();
        assert_eq!(doc.history().undo_len(), 1);
        let cli = ExecCtx::new(Origin::Cli, 600);
        doc.execute(&cli, &width(0, 3.0)).unwrap();
        doc.execute(&cli, &width(0, 4.0)).unwrap();
        assert_eq!(doc.history().undo_len(), 3);
        // Deletes never coalesce.
        let delete = |id| {
            Command::GlyphDelete(GlyphDelete {
                ids: vec![GlyphId(id)],
            })
        };
        doc.execute(&user(700), &delete(3)).unwrap();
        doc.execute(&user(701), &delete(2)).unwrap();
        assert_eq!(doc.history().undo_len(), 5);
    }

    #[test]
    fn a_transaction_ends_the_coalescing_run() {
        let mut doc = Document::new(fixture::font());
        doc.execute(&user(0), &width(0, 1.0)).unwrap();
        let txn = doc.begin("t").unwrap();
        doc.rollback(txn).unwrap();
        doc.execute(&user(100), &width(0, 2.0)).unwrap();
        assert_eq!(doc.history().undo_len(), 2);
    }

    /// A glyph ID handed out once is not handed out again after an undo or a rollback.
    #[test]
    fn undo_never_lets_a_glyph_id_be_reused() {
        let mut doc = Document::new(fixture::font());
        let created = |outcome: Outcome| match outcome.events.as_slice() {
            [Event::GlyphCreated { id, .. }] => *id,
            other => panic!("{other:?}"),
        };
        let b = created(doc.execute(&user(0), &create("B")).unwrap());
        doc.execute(&user(1), &UNDO).unwrap();
        let c = created(doc.execute(&user(2), &create("C")).unwrap());
        assert_ne!(b, c);
        assert!(doc.state().font.glyph(b).is_none());

        let txn = doc.begin("t").unwrap();
        let mut ctx = user(3);
        ctx.txn = Some(txn);
        let d = created(doc.execute(&ctx, &create("D")).unwrap());
        doc.rollback(txn).unwrap();
        let e = created(doc.execute(&user(4), &create("E")).unwrap());
        assert_ne!(d, e);

        // Redo still finds the glyph at the ID it had.
        doc.execute(&user(5), &UNDO).unwrap();
        doc.execute(&user(6), &REDO).unwrap();
        assert_eq!(doc.state().font.glyphs().id_of("E"), Some(e));
    }
}

/// Property tests of the history over generated command sequences (Step 4 *Tasks*).
#[cfg(test)]
mod properties {
    use proptest::prelude::*;
    use tf_commands::command::{DocRedo, DocUndo, GlyphCreate};

    use super::*;
    use crate::history::COALESCE_WINDOW_MS;
    use crate::reduce::testing::{CommandGen, command};

    /// One step of a generated session.
    #[derive(Clone, Debug)]
    enum Op {
        /// A command, from the user or not, `dt` ms after the previous op.
        Run {
            command: CommandGen,
            user: bool,
            dt: u16,
        },
        Undo,
        Redo,
        Begin,
        Commit,
        Rollback,
    }

    fn op() -> impl Strategy<Value = Op> {
        prop_oneof![
            8 => (command(), any::<bool>(), 0u16..1000)
                .prop_map(|(command, user, dt)| Op::Run { command, user, dt }),
            2 => Just(Op::Undo),
            1 => Just(Op::Redo),
            1 => Just(Op::Begin),
            1 => Just(Op::Commit),
            1 => Just(Op::Rollback),
        ]
    }

    fn ops() -> impl Strategy<Value = Vec<Op>> {
        prop::collection::vec(op(), 0..40)
    }

    /// Commands only: single commands and drags (one command repeated by the user with new
    /// values, mostly inside the coalescing window), so that runs join and split.
    fn runs() -> impl Strategy<Value = Vec<Op>> {
        let single = (command(), any::<bool>(), 0u16..1000)
            .prop_map(|(command, user, dt)| vec![Op::Run { command, user, dt }]);
        let drag = (
            command(),
            prop::collection::vec((any::<i16>(), 0u16..700), 2..6),
        )
            .prop_map(|(command, moves)| {
                moves
                    .into_iter()
                    .map(|(value, dt)| {
                        let mut command = command.clone();
                        command.pick.value = value;
                        Op::Run {
                            command,
                            user: true,
                            dt,
                        }
                    })
                    .collect::<Vec<_>>()
            });
        prop::collection::vec(prop_oneof![single, drag], 0..8)
            .prop_map(|bursts| bursts.into_iter().flatten().collect())
    }

    /// Drives a document with ops, ignoring their errors, and checks that the revision
    /// moves by one per change and that failures and no-ops change neither the state nor
    /// the history.
    struct Session {
        doc: Document,
        at_ms: u64,
        /// State changes of any op (commands, undo, redo, rollback).
        changes: usize,
        /// Commands that changed the state.
        commits: usize,
        /// Commands outside a transaction that joined the previous undo step.
        joins: usize,
        /// Transaction commits that recorded a step.
        txn_steps: usize,
    }

    /// What the history looks like from outside.
    type HistoryView = (usize, usize, Option<String>, Option<String>, Option<TxnId>);

    impl Session {
        fn new(font: Font, cap: usize) -> Self {
            Self {
                doc: Document::with_history_cap(font, cap),
                at_ms: 0,
                changes: 0,
                commits: 0,
                joins: 0,
                txn_steps: 0,
            }
        }

        fn view(&self) -> HistoryView {
            let h = self.doc.history();
            (
                h.undo_len(),
                h.redo_len(),
                h.undo_label().map(str::to_owned),
                h.redo_label().map(str::to_owned),
                self.doc.transaction(),
            )
        }

        fn ctx(&self, origin: Origin) -> ExecCtx {
            ExecCtx {
                txn: self.doc.transaction(),
                ..ExecCtx::new(origin, self.at_ms)
            }
        }

        fn run(&mut self, op: &Op) -> Result<(), TestCaseError> {
            let before = Arc::clone(self.doc.state());
            let view = self.view();
            let result = match op {
                Op::Run { command, user, dt } => {
                    self.at_ms += u64::from(*dt);
                    let origin = if *user { Origin::User } else { Origin::Cli };
                    let ctx = self.ctx(origin);
                    let command = command.build(&self.doc.state().font);
                    self.doc.execute(&ctx, &command)
                }
                Op::Undo => self
                    .doc
                    .execute(&self.ctx(Origin::User), &Command::DocUndo(DocUndo {})),
                Op::Redo => self
                    .doc
                    .execute(&self.ctx(Origin::User), &Command::DocRedo(DocRedo {})),
                Op::Begin => self.doc.begin("generated").map(|_| self.outcome()),
                Op::Commit => self.close(Document::commit),
                Op::Rollback => self.close(Document::rollback),
            };
            let after = Arc::clone(self.doc.state());
            let changed = after.revision != before.revision;
            match &result {
                Ok(outcome) => {
                    prop_assert_eq!(outcome.revision, after.revision);
                    if changed {
                        prop_assert_eq!(outcome.revision, before.revision.next());
                        self.changes += 1;
                    } else {
                        prop_assert!(Arc::ptr_eq(&before, &after));
                        if !matches!(op, Op::Begin | Op::Commit | Op::Rollback) {
                            prop_assert_eq!(&self.view(), &view);
                        }
                    }
                }
                Err(_) => {
                    prop_assert!(Arc::ptr_eq(&before, &after));
                    prop_assert_eq!(&self.view(), &view);
                }
            }
            if result.is_ok() {
                match op {
                    Op::Run { .. } if changed => {
                        self.commits += 1;
                        if view.4.is_none() && self.view().0 == view.0 {
                            self.joins += 1;
                        }
                    }
                    Op::Commit if self.view().0 > view.0 => self.txn_steps += 1,
                    _ => {}
                }
            }
            Ok(())
        }

        fn run_all(&mut self, ops: &[Op]) -> Result<(), TestCaseError> {
            ops.iter().try_for_each(|op| self.run(op))
        }

        fn close(
            &mut self,
            close: fn(&mut Document, TxnId) -> Result<Outcome, CommandError>,
        ) -> Result<Outcome, CommandError> {
            let id = self.doc.transaction().unwrap_or(TxnId(u32::MAX));
            close(&mut self.doc, id)
        }

        fn outcome(&self) -> Outcome {
            Outcome {
                revision: self.doc.state().revision,
                events: Vec::new(),
                touched: BTreeSet::new(),
            }
        }

        /// Commits the open transaction, if any.
        fn settle(&mut self) {
            if let Some(id) = self.doc.transaction() {
                let _ = self.doc.commit(id);
            }
        }

        fn font(&self) -> Font {
            self.doc.state().font.clone()
        }

        fn undo(&mut self) -> Result<Outcome, CommandError> {
            self.doc
                .execute(&self.ctx(Origin::User), &Command::DocUndo(DocUndo {}))
        }

        fn redo(&mut self) -> Result<Outcome, CommandError> {
            self.doc
                .execute(&self.ctx(Origin::User), &Command::DocRedo(DocRedo {}))
        }
    }

    /// Larger than any generated session, so nothing is dropped.
    const ROOMY: usize = 1000;

    /// The properties below mean something only if generated sessions change the document:
    /// most commands must target live glyphs and succeed.
    #[test]
    fn generated_sessions_change_the_document() {
        use proptest::strategy::ValueTree;
        use proptest::test_runner::TestRunner;

        let mut runner = TestRunner::deterministic();
        let (mut commands, mut commits, mut joins, mut txn_steps, mut undone) = (0, 0, 0, 0, 0);
        for _ in 0..100 {
            let (font, ops, runs) = (tf_model::testing::font(), ops(), runs())
                .new_tree(&mut runner)
                .unwrap()
                .current();
            let mut s = Session::new(font, ROOMY);
            s.run_all(&ops).unwrap();
            s.settle();
            s.run_all(&runs).unwrap();
            commands += runs.len();
            commands += ops.iter().filter(|op| matches!(op, Op::Run { .. })).count();
            commits += s.commits;
            joins += s.joins;
            txn_steps += s.txn_steps;
            s.settle();
            while s.undo().is_ok() {
                undone += 1;
            }
        }
        assert!(
            commits * 2 > commands,
            "{commits} of {commands} commands changed the font"
        );
        assert!(joins >= 20, "only {joins} coalesced commits");
        assert!(
            txn_steps >= 10,
            "only {txn_steps} transactions recorded a step"
        );
        assert!(undone > 100, "only {undone} steps to undo");
    }

    proptest! {
        #[test]
        fn undo_then_redo_and_redo_then_undo_are_identities(
            font in tf_model::testing::font(),
            ops in ops(),
        ) {
            let mut s = Session::new(font, ROOMY);
            s.run_all(&ops)?;
            s.settle();
            let now = s.font();
            if s.undo().is_ok() {
                s.redo().map_err(|e| TestCaseError::fail(e.to_string()))?;
                prop_assert_eq!(s.font(), now.clone());
            }
            if s.doc.history().redo_len() > 0 {
                s.redo().map_err(|e| TestCaseError::fail(e.to_string()))?;
                s.undo().map_err(|e| TestCaseError::fail(e.to_string()))?;
                prop_assert_eq!(s.font(), now);
            }
        }

        #[test]
        fn undoing_everything_returns_the_initial_state(
            font in tf_model::testing::font(),
            ops in ops(),
        ) {
            let mut s = Session::new(font.clone(), ROOMY);
            s.run_all(&ops)?;
            s.settle();
            while s.undo().is_ok() {}
            prop_assert_eq!(s.font(), font);
        }

        #[test]
        fn a_commit_after_undo_empties_redo(
            font in tf_model::testing::font(),
            ops in ops(),
        ) {
            let mut s = Session::new(font, ROOMY);
            s.run_all(&ops)?;
            s.settle();
            if s.undo().is_ok() {
                prop_assert!(s.doc.history().redo_len() > 0);
                // A name longer than any generated one is always free.
                let create = Command::GlyphCreate(GlyphCreate {
                    name: "fresh.glyph.name".into(),
                    codepoints: vec![],
                    width: 0.0,
                });
                s.doc
                    .execute(&s.ctx(Origin::User), &create)
                    .map_err(|e| TestCaseError::fail(e.to_string()))?;
                prop_assert_eq!(s.doc.history().redo_len(), 0);
            }
        }

        /// The number of undo steps matches a reference model of coalescing: a commit joins
        /// the previous one only when both are `User` commits with the same key and at most
        /// 500 ms apart.
        #[test]
        fn coalescing_follows_keys_origins_and_the_window(
            font in tf_model::testing::font(),
            runs in runs(),
        ) {
            let mut s = Session::new(font, ROOMY);
            let mut expected = 0usize;
            let mut run: Option<(CoalesceKey, u64)> = None;
            for op in &runs {
                let Op::Run { command, user, .. } = op else { unreachable!() };
                let built = command.build(&s.doc.state().font);
                let revision = s.doc.state().revision;
                s.run(op)?;
                if s.doc.state().revision == revision {
                    continue; // failed or a no-op: no commit
                }
                let key = (*user).then(|| CoalesceKey::of(&built)).flatten();
                let at = s.at_ms;
                let joins = matches!(
                    (run, key),
                    (Some((k0, t0)), Some(k)) if k == k0
                        && at.checked_sub(t0).is_some_and(|gap| gap <= COALESCE_WINDOW_MS)
                );
                if !joins {
                    expected += 1;
                }
                run = key.map(|k| (k, at));
                prop_assert_eq!(s.doc.history().undo_len(), expected);
            }
            prop_assert_eq!(s.doc.history().undo_len(), expected);
        }

        #[test]
        fn a_transaction_is_one_undo_step_and_rollback_restores_begin(
            font in tf_model::testing::font(),
            before in ops(),
            inside in runs(),
            commit in any::<bool>(),
        ) {
            let mut s = Session::new(font, ROOMY);
            s.run_all(&before)?;
            s.settle();
            let at_begin = s.font();
            let steps = s.doc.history().undo_len();
            let redo = s.doc.history().redo_len();
            let id = s.doc.begin("generated").map_err(|e| TestCaseError::fail(e.to_string()))?;
            let changes = s.changes;
            s.run_all(&inside)?;
            let changed = s.changes > changes;
            if commit {
                s.doc.commit(id).map_err(|e| TestCaseError::fail(e.to_string()))?;
                prop_assert_eq!(s.doc.history().undo_len(), steps + usize::from(changed));
                if changed {
                    prop_assert_eq!(s.doc.history().redo_len(), 0);
                    s.undo().map_err(|e| TestCaseError::fail(e.to_string()))?;
                }
            } else {
                s.doc.rollback(id).map_err(|e| TestCaseError::fail(e.to_string()))?;
                prop_assert_eq!(s.doc.history().undo_len(), steps);
                prop_assert_eq!(s.doc.history().redo_len(), redo);
            }
            prop_assert_eq!(s.font(), at_begin);
        }

        /// With a small cap, the stack keeps the newest steps: undoing all of them lands on
        /// the state after the oldest dropped step.
        #[test]
        fn the_cap_drops_the_oldest_step(
            font in tf_model::testing::font(),
            runs in runs(),
            cap in 1usize..6,
        ) {
            let mut s = Session::new(font.clone(), cap);
            let mut states = vec![font];
            for op in &runs {
                let Op::Run { command, dt, .. } = op else { unreachable!() };
                // Not from the user, so every commit is its own step.
                let cli = Op::Run { command: command.clone(), user: false, dt: *dt };
                let revision = s.doc.state().revision;
                s.run(&cli)?;
                if s.doc.state().revision != revision {
                    states.push(s.font());
                }
            }
            let commits = states.len() - 1;
            prop_assert_eq!(s.doc.history().undo_len(), commits.min(cap));
            while s.undo().is_ok() {}
            prop_assert_eq!(s.font(), states[commits.saturating_sub(cap)].clone());
        }
    }
}
