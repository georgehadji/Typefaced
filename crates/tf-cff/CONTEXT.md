# crates/tf-cff/

Crate `tf-cff`, layer `domain`: a minimal CFF version 1 writer. Cubic outlines and
advances in, `CFF ` table bytes out; no I/O (ADR-0007). `tf-compile` uses it to turn
fontc's TTF into an OTF. Up: [crates/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `Cargo.toml` | Crate manifest: `kurbo` and `read-fonts` (exact pins, the versions in fontc's tree; read-fonts only for the CFF standard strings) and `thiserror`; `skrifa` and `write-fonts` for tests |

| Subfolder | Context |
|---|---|
| `src/` | [CONTEXT.md](src/CONTEXT.md): the builder, the outline and charstring encoder, the byte encodings |
| `tests/` | [CONTEXT.md](tests/CONTEXT.md): tables built and read back with read-fonts and skrifa |
