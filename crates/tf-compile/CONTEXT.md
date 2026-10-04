# crates/tf-compile/

Crate `tf-compile`, layer `adapter`: compiles UFO and designspace sources to TrueType
with fontc, linked in-process (ADR-0006), and turns a static TTF into a CFF-based OTF
with a `CFF ` table from `tf-cff` (ADR-0007). Up: [crates/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `Cargo.toml` | Crate manifest: depends on `fontc` (exact version pin, default features off, `rayon` on), `write-fonts`, `kurbo` and `norad` (exact pins, the versions in fontc's tree; norad with its `kurbo` feature), `tf-cff` and `thiserror`; `skrifa` for tests |

| Subfolder | Context |
|---|---|
| `src/` | [CONTEXT.md](src/CONTEXT.md): the compile functions, the OTF transplant, the source outline reader and the error type |
| `examples/` | [CONTEXT.md](examples/CONTEXT.md): timing programs for Spikes 1 and 2 |
| `tests/` | [CONTEXT.md](tests/CONTEXT.md): fixture, corpus, designspace 5 and OTF tests |
