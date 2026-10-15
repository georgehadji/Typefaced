# crates/tf-model/

Crate `tf-model`, layer `domain`: the domain model of a font project (implementation plan
§5.1) on persistent `imbl` collections (ADR-0003, ADR-0017). Pure: no I/O, no serde. The
crate-level rustdoc in `src/lib.rs` holds the inventory table that says where every norad
0.18.4 field lives (the contract for `tf-io`). Up: [crates/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `Cargo.toml` | Crate manifest: `imbl` (used unmodified, MPL-2.0), `indexmap`, `smallvec`, `thiserror`; feature `testing` exposes the proptest generators to other crates; two `harness = false` benches |

| Subfolder | Context |
|---|---|
| `benches/` | [CONTEXT.md](benches/CONTEXT.md): glyph-table and kerning budgets |
| `src/` | [CONTEXT.md](src/CONTEXT.md): the model types, glyph table, checks and generators |
