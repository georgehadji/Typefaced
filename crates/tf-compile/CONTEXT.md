# crates/tf-compile/

Crate `tf-compile`, layer `adapter`: compiles UFO and designspace sources to TrueType
with fontc, linked in-process (ADR-0006). Up: [crates/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `Cargo.toml` | Crate manifest: depends on `fontc` (exact version pin, default features off, `rayon` on) and `thiserror`; `skrifa` for tests |

| Subfolder | Context |
|---|---|
| `src/` | [CONTEXT.md](src/CONTEXT.md): the compile function and its error type |
| `examples/` | [CONTEXT.md](examples/CONTEXT.md): timing programs for Spike 1 |
| `tests/` | [CONTEXT.md](tests/CONTEXT.md): fixture, corpus and designspace 5 tests |
