# crates/tf-boolean/

Crate `tf-boolean`, layer `adapter`: the `BooleanEngine` port of `tf-geometry` on
linesweeper, pinned `=0.4.0` (ADR-0008). It also holds the regression corpus that every
linesweeper upgrade reruns: the 581 cases of Spike 5, checked against an
engine-independent scanline oracle. Up: [crates/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `Cargo.toml` | Crate manifest: depends on `tf-geometry`, `kurbo` and `linesweeper`; `norad` (reads the fixture and corpus glyphs) and `proptest` as dev-dependencies; the `ops` bench |

| Subfolder | Context |
|---|---|
| `src/` | [CONTEXT.md](src/CONTEXT.md): the adapter |
| `tests/` | [CONTEXT.md](tests/CONTEXT.md): regression corpus, thin-feature, debris and property tests |
| `benches/` | [CONTEXT.md](benches/CONTEXT.md): timing of remove-overlap, intersection and difference |

Upgrade check (ADR-0008): `cargo xtask corpus fetch`, then
`cargo test -p tf-boolean --locked` (33 cases) and
`cargo test -p tf-boolean --locked -- --ignored --nocapture` (548 cases). Each run prints one
summary line per group: cases, crashes, errors, failing cases, worst area error.
