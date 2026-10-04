# crates/tf-wasm/src/

Up: [tf-wasm/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `lib.rs` | `wasm-bindgen` exports `hitTest` and `translatePoints`, thin wrappers over `tf-geometry` that take typed arrays; `hitTest` encodes its result as a flat `Float64Array`. Test checks that encoding |
