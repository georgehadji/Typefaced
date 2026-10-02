# crates/tf-geometry/

Crate `tf-geometry`, layer `domain`: the geometry kernel on packed outlines (hit testing
and point edits). Pure functions, no I/O, and it compiles to `wasm32-unknown-unknown`;
the `tf-wasm` crate exposes it to the UI. Up: [crates/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `Cargo.toml` | Crate manifest: depends on `kurbo` for curve math; `proptest` for property tests (dev-dependency) |

| Subfolder | Context |
|---|---|
| `src/` | [CONTEXT.md](src/CONTEXT.md): the kernel |
