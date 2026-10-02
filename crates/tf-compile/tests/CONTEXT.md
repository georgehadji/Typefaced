# crates/tf-compile/tests/

Integration tests. The corpus tests are marked `#[ignore]` and need the pinned corpus:
`cargo xtask corpus fetch`, then `cargo test -p tf-compile -- --ignored`.
Up: [tf-compile/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `compile_fixture.rs` | Compiles `tests/fixtures/min.ufo` and checks the TTF with skrifa: glyph count, `cmap`, the contours of `A`. Also checks the error cases: unsupported extension, a fontc panic, a missing source |
| `corpus.rs` | Corpus-backed checks: every style of the static family compiles, and the variable family fails on its per-master feature files but compiles with a weight axis once they are removed |
| `designspace_v5.rs` | Writes small designspace 5 documents and pins what fontc makes of each feature (discrete axis, labels, mappings, variable-fonts, instances). A changed outcome after a fontc upgrade fails on purpose |

| Subfolder | Context |
|---|---|
| `data/` | [CONTEXT.md](data/CONTEXT.md): input files for the tests |
