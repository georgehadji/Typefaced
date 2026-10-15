# crates/tf-boolean/tests/

Up: [tf-boolean/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `regression.rs` | Remove-overlap on the regression corpus, each output checked for errors, panics, open contours, non-finite points, overlaps left, mixed winding and area error < 0.1% against the oracle: 29 synthetic and 4 fixture cases always, the 548 Source Sans cases as an `#[ignore]` test (needs `cargo xtask corpus fetch`). Also the oracle's own checks, the thin-feature check (each sliver measured on its own) and the debris check |
| `properties.rs` | Property tests of the area invariants of union, intersection and difference on generated pairs of shapes (§5.2) |

| Subfolder | Context |
|---|---|
| `common/` | [CONTEXT.md](common/CONTEXT.md): case set and scanline oracle shared by the tests and the bench |
