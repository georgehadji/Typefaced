//! The execution context of a command (engine-internal, not a wire type): who sent it, in
//! which transaction and sandbox, against which revision, and when.

use tf_commands::query::SandboxId;

/// A document revision. It grows by one on every change of the state (a command, an undo,
/// a redo, a rollback) and is never reused, so a client can tell whether its view is
/// current.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Revision(pub u32);

impl Revision {
    /// The revision after this one.
    #[must_use]
    pub fn next(self) -> Self {
        // ponytail: saturates after 4 billion changes; stale-base checks stop telling
        // revisions apart then. A u64 on the engine side would fix it if it ever matters.
        Self(self.0.saturating_add(1))
    }
}

/// An open transaction, as returned by [`Document::begin`](crate::Document::begin).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TxnId(pub u32);

/// Who sent a command. Only [`Origin::User`] commits coalesce into one undo step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    /// The person at the UI.
    User,
    /// An AI agent session.
    Ai { session: u32 },
    /// An MCP client.
    Mcp { client: u32 },
    /// The command-line tool.
    Cli,
}

/// How one command runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecCtx {
    pub origin: Origin,
    /// The open transaction the command belongs to. While a transaction is open, every
    /// command must name it (`E_TRANSACTION_OPEN` otherwise); naming one that is not open
    /// fails with `E_NO_TRANSACTION`.
    pub txn: Option<TxnId>,
    /// The sandbox to run in. Sandboxes come with Step 7: until then any value fails with
    /// `E_SANDBOX_NOT_FOUND`.
    pub sandbox: Option<SandboxId>,
    /// The revision the sender saw. When set and not current, the command fails with
    /// `E_STALE_BASE`.
    pub base: Option<Revision>,
    /// When the command was sent, in milliseconds (plan deviation 2: time is an input; the
    /// actor reads the clock, tests pass fixed values). Coalescing compares it.
    pub at_ms: u64,
}

impl ExecCtx {
    /// A context with no transaction, sandbox or base.
    pub fn new(origin: Origin, at_ms: u64) -> Self {
        Self {
            origin,
            txn: None,
            sandbox: None,
            base: None,
            at_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revisions_grow_and_saturate() {
        assert_eq!(Revision(4).next(), Revision(5));
        assert_eq!(Revision(u32::MAX).next(), Revision(u32::MAX));
        let ctx = ExecCtx::new(Origin::Ai { session: 2 }, 7);
        assert_eq!(
            (ctx.txn, ctx.sandbox, ctx.base, ctx.at_ms),
            (None, None, None, 7)
        );
    }
}
