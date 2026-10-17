//! The document engine's functional core (implementation plan §5.12, ADR-0003): pure
//! reducers over a persistent [`DocState`], and a synchronous [`Document`] that owns one
//! document's state, its undo/redo history and its open transaction. The actor (Step 8)
//! wraps a `Document`; this crate has no threads and no I/O.
//!
//! - [`ctx`]: the execution context of a command ([`ExecCtx`]: origin, transaction,
//!   sandbox, base revision, time);
//! - [`document`]: [`Document`], [`DocState`] and the outcome of a command;
//! - [`history`]: undo/redo stacks with coalescing and a cap;
//! - [`reduce`]: the dispatch from a catalog [`Command`](tf_commands::Command) to its
//!   reducer, one file per command family;
//! - `map`: DTO ↔ model conversions and model errors → stable error codes.

pub mod ctx;
pub mod document;
pub mod history;
mod map;
pub mod reduce;

pub use ctx::{ExecCtx, Origin, Revision, TxnId};
pub use document::{DocState, Document, Outcome};
pub use history::{CoalesceKey, DEFAULT_CAP, History};
pub use reduce::{Event, Reduced, Touched, reduce};
