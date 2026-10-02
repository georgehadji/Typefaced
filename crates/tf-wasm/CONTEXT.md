# crates/tf-wasm/

Crate `tf-wasm`, layer `driver`: the WebAssembly facade over `tf-geometry` for the UI,
built by `wasm-pack` into `packages/geometry-wasm/pkg`. Up: [crates/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `Cargo.toml` | Crate manifest: `cdylib` + `rlib`; depends on `tf-geometry` and `wasm-bindgen`; the wasm-opt flags that `wasm-pack` needs for the release profile |

| Subfolder | Context |
|---|---|
| `src/` | [CONTEXT.md](src/CONTEXT.md): the exported functions |
