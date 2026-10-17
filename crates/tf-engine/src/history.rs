//! Undo and redo stacks of `Arc<DocState>` roots with labels (ADR-0003: snapshot undo on
//! persistent state). A step stores the state *before* it, so undo is a pointer swap.
//!
//! - A new commit clears redo.
//! - The undo stack holds at most `cap` steps ([`DEFAULT_CAP`]); the oldest goes first.
//! - Coalescing: a `User` commit with the same [`CoalesceKey`] as the previous commit, at
//!   most [`COALESCE_WINDOW_MS`] after it, joins that commit's step. Undo, redo, a
//!   transaction, a commit from another origin or with another key ends the run.

use std::collections::{BTreeSet, VecDeque};
use std::sync::Arc;

use tf_commands::{Command, CommandId};

use crate::document::DocState;
use crate::reduce::Touched;

/// The default number of undo steps (implementation plan §10.4).
pub const DEFAULT_CAP: usize = 200;

/// The longest gap between two commits that still coalesce.
pub const COALESCE_WINDOW_MS: u64 = 500;

/// What makes two commits one undo step: the command and its targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CoalesceKey {
    pub command: CommandId,
    pub glyph: u32,
    pub layer: u32,
    /// The anchor of `glyph.moveAnchor`.
    pub anchor: Option<u32>,
}

impl CoalesceKey {
    /// The key of a command that coalesces (`glyph.movePoints`, `glyph.transform`,
    /// `glyph.moveAnchor`, `glyph.setWidth`); `None` for every other command.
    pub fn of(command: &Command) -> Option<Self> {
        let key = |glyph: u32, layer: u32, anchor| CoalesceKey {
            command: command.id(),
            glyph,
            layer,
            anchor,
        };
        match command {
            Command::GlyphMovePoints(p) => Some(key(p.id.0, p.layer.0, None)),
            Command::GlyphTransform(p) => Some(key(p.id.0, p.layer.0, None)),
            Command::GlyphMoveAnchor(p) => Some(key(p.id.0, p.layer.0, Some(p.index))),
            Command::GlyphSetWidth(p) => Some(key(p.id.0, p.layer.0, None)),
            _ => None,
        }
    }
}

/// One undo or redo step: the state to go back to, a label and what the step changed.
#[derive(Clone, Debug)]
pub(crate) struct Step {
    pub state: Arc<DocState>,
    pub label: String,
    pub touched: BTreeSet<Touched>,
}

/// The undo and redo stacks of one document.
#[derive(Clone, Debug)]
pub struct History {
    undo: VecDeque<Step>,
    redo: Vec<Step>,
    cap: usize,
    /// The key and time of the last commit, while it may still coalesce.
    run: Option<(CoalesceKey, u64)>,
}

impl History {
    /// Empty stacks that keep at most `cap` undo steps.
    pub fn new(cap: usize) -> Self {
        Self {
            undo: VecDeque::new(),
            redo: Vec::new(),
            cap,
            run: None,
        }
    }

    /// The number of undo steps.
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    /// The number of redo steps.
    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    /// The label of the step that undo would revert.
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.back().map(|step| step.label.as_str())
    }

    /// The label of the step that redo would apply.
    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|step| step.label.as_str())
    }

    /// Records a commit whose state before was `before`. `coalesce` is the commit's key and
    /// time when it may join the previous step (a `User` commit with a key).
    pub(crate) fn record(
        &mut self,
        before: Arc<DocState>,
        label: String,
        touched: BTreeSet<Touched>,
        coalesce: Option<(CoalesceKey, u64)>,
    ) {
        self.redo.clear();
        let joins = match (self.run, coalesce) {
            (Some((last_key, last_at)), Some((key, at))) => {
                key == last_key
                    && at
                        .checked_sub(last_at)
                        .is_some_and(|gap| gap <= COALESCE_WINDOW_MS)
            }
            _ => false,
        };
        self.run = coalesce;
        if joins && let Some(top) = self.undo.back_mut() {
            // The step keeps the state before the first commit of the run.
            top.touched.extend(touched);
            return;
        }
        self.undo.push_back(Step {
            state: before,
            label,
            touched,
        });
        while self.undo.len() > self.cap {
            self.undo.pop_front();
        }
        if self.undo.is_empty() {
            // A cap of 0 keeps nothing, so nothing can coalesce either.
            self.run = None;
        }
    }

    /// Pops the last undo step and moves `current` onto the redo stack in its place.
    pub(crate) fn undo(&mut self, current: Arc<DocState>) -> Option<Step> {
        let step = self.undo.pop_back()?;
        self.redo.push(Step {
            state: current,
            label: step.label.clone(),
            touched: step.touched.clone(),
        });
        self.run = None;
        Some(step)
    }

    /// Pops the last redo step and moves `current` onto the undo stack in its place.
    pub(crate) fn redo(&mut self, current: Arc<DocState>) -> Option<Step> {
        let step = self.redo.pop()?;
        self.undo.push_back(Step {
            state: current,
            label: step.label.clone(),
            touched: step.touched.clone(),
        });
        self.run = None;
        Some(step)
    }
}

#[cfg(test)]
mod tests {
    use tf_commands::command::{GlyphCreate, GlyphId, GlyphMoveAnchor, LayerId};
    use tf_model::GlyphId as ModelGlyphId;

    use super::*;
    use crate::Revision;
    use crate::reduce::fixture;

    fn state(revision: u32) -> Arc<DocState> {
        Arc::new(DocState {
            font: fixture::font(),
            revision: Revision(revision),
        })
    }

    fn touched(glyph: u32) -> BTreeSet<Touched> {
        [Touched::Glyph(ModelGlyphId(glyph))].into()
    }

    fn key(glyph: u32) -> CoalesceKey {
        CoalesceKey {
            command: CommandId::GlyphSetWidth,
            glyph,
            layer: 0,
            anchor: None,
        }
    }

    fn record(history: &mut History, revision: u32, coalesce: Option<(CoalesceKey, u64)>) {
        history.record(
            state(revision),
            format!("step {revision}"),
            touched(revision),
            coalesce,
        );
    }

    #[test]
    fn only_the_four_drag_commands_coalesce() {
        let anchor = Command::GlyphMoveAnchor(GlyphMoveAnchor {
            id: GlyphId(1),
            layer: LayerId(2),
            index: 3,
            x: 0.0,
            y: 0.0,
        });
        assert_eq!(
            CoalesceKey::of(&anchor),
            Some(CoalesceKey {
                command: CommandId::GlyphMoveAnchor,
                glyph: 1,
                layer: 2,
                anchor: Some(3)
            })
        );
        let create = Command::GlyphCreate(GlyphCreate {
            name: "a".into(),
            codepoints: vec![],
            width: 0.0,
        });
        assert_eq!(CoalesceKey::of(&create), None);
        // Every example of the registry: exactly the four commands have a key.
        let keyed: Vec<CommandId> = CommandId::ALL
            .iter()
            .filter_map(|&id| {
                let example = tf_commands::registry::command(id).example;
                let command: Command = serde_json::from_str(example).unwrap();
                CoalesceKey::of(&command).map(|_| id)
            })
            .collect();
        assert_eq!(
            keyed,
            [
                CommandId::GlyphSetWidth,
                CommandId::GlyphMovePoints,
                CommandId::GlyphTransform,
                CommandId::GlyphMoveAnchor
            ]
        );
    }

    #[test]
    fn the_window_is_measured_from_the_previous_commit() {
        let mut history = History::new(DEFAULT_CAP);
        record(&mut history, 0, Some((key(0), 1_000)));
        record(&mut history, 1, Some((key(0), 1_500))); // gap 500: joins
        record(&mut history, 2, Some((key(0), 1_900))); // gap 400 from the previous: joins
        assert_eq!(history.undo_len(), 1);
        record(&mut history, 3, Some((key(0), 2_401))); // gap 501: new step
        assert_eq!(history.undo_len(), 2);
        // The joined step keeps the first state before the run and every touched entity.
        let first = &history.undo[0];
        assert_eq!(first.state.revision, Revision(0));
        assert_eq!(first.touched, (0..3).flat_map(touched).collect());
        assert_eq!(history.undo_label(), Some("step 3"));
    }

    #[test]
    fn other_keys_origins_undo_and_clock_steps_back_end_the_run() {
        let mut history = History::new(DEFAULT_CAP);
        record(&mut history, 0, Some((key(0), 0)));
        record(&mut history, 1, Some((key(1), 10))); // another key
        record(&mut history, 2, None); // not a User commit with a key
        record(&mut history, 3, Some((key(1), 20)));
        assert_eq!(history.undo_len(), 4);
        history.undo(state(9)).unwrap();
        record(&mut history, 4, Some((key(1), 30))); // after undo: new step
        assert_eq!(history.undo_len(), 4);
        record(&mut history, 5, Some((key(1), 25))); // the clock went back: new step
        assert_eq!(history.undo_len(), 5);
    }

    #[test]
    fn undo_and_redo_swap_steps_and_a_commit_clears_redo() {
        let mut history = History::new(DEFAULT_CAP);
        assert!(history.undo(state(0)).is_none());
        assert!(history.redo(state(0)).is_none());
        record(&mut history, 0, None);
        record(&mut history, 1, None);
        let step = history.undo(state(2)).unwrap();
        assert_eq!(step.state.revision, Revision(1));
        assert_eq!((history.undo_len(), history.redo_len()), (1, 1));
        assert_eq!(history.redo_label(), Some("step 1"));
        let step = history.redo(state(3)).unwrap();
        assert_eq!(step.state.revision, Revision(2));
        assert_eq!(history.undo.back().unwrap().state.revision, Revision(3));
        history.undo(state(4)).unwrap();
        record(&mut history, 5, None);
        assert_eq!(history.redo_len(), 0);
        assert_eq!(history.redo_label(), None);
    }

    #[test]
    fn the_cap_drops_the_oldest_step() {
        let mut history = History::new(2);
        for revision in 0..5 {
            record(&mut history, revision, None);
        }
        assert_eq!(history.undo_len(), 2);
        assert_eq!(history.undo[0].state.revision, Revision(3));
        // Joining the top step does not grow the stack.
        record(&mut history, 5, Some((key(0), 0)));
        record(&mut history, 6, Some((key(0), 1)));
        assert_eq!(history.undo_len(), 2);

        let mut none = History::new(0);
        record(&mut none, 0, Some((key(0), 0)));
        record(&mut none, 1, Some((key(0), 1)));
        assert_eq!(none.undo_len(), 0);
        assert_eq!(none.undo_label(), None);
    }
}
