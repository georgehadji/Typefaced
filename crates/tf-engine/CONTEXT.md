# crates/tf-engine/

Crate `tf-engine`, layer `application`: the document engine (implementation plan §5.12,
ADR-0003). Pure reducers turn a catalog command into the next `DocState`; a synchronous
`Document` owns one document's state, its undo/redo history and its open transaction.
No threads and no I/O: the actor (M1 Step 8) wraps a `Document`. Depends inward only, on
`tf-model` and `tf-commands`. Up: [crates/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `Cargo.toml` | Crate manifest: `tf-model` and `tf-commands`; `proptest`, `serde_json` and `tf-model`'s `testing` generators for tests |

| Subfolder | Context |
|---|---|
| `src/` | [CONTEXT.md](src/CONTEXT.md): document, history, context, DTO mapping, reducers |
