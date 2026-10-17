# crates/tf-engine/src/

Up: [tf-engine/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `lib.rs` | Crate docs, module list and re-exports |
| `ctx.rs` | `ExecCtx` (origin, transaction, sandbox, base revision, `at_ms`), `Origin`, `Revision`, `TxnId` |
| `document.rs` | `Document`: runs commands (context checks, reducer, history), `doc.undo`/`doc.redo`, transactions (`begin`, `commit`, `rollback`); `DocState` and `Outcome`. Holds the history property tests over generated command sequences |
| `history.rs` | `History`: undo/redo stacks of `Arc<DocState>` with labels and touched entities, the cap (200 by default) and coalescing (`CoalesceKey`, 500 ms window, `User` commits only) |
| `map.rs` | DTO → model conversions (IDs, names, code points, finite units) and `ModelError` → stable error code |

| Subfolder | Context |
|---|---|
| `reduce/` | [CONTEXT.md](reduce/CONTEXT.md): the dispatch and one reducer file per command family |
