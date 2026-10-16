# crates/tf-boolean/tests/

Up: [tf-boolean/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `regression.rs` | Remove-overlap on the regression corpus, each output checked for errors, panics, open contours, non-finite points, overlaps left, clockwise-filled area (mixed or reversed direction) and area error < 0.1% against the oracle: 29 synthetic and 4 fixture cases always, the 548 Source Sans cases as an `#[ignore]` test (needs `cargo xtask corpus fetch`). Also the oracle's own checks, the thin-feature check (each sliver measured on its own) and the debris check, also at ten times the size |
| `properties.rs` | Property tests (§5.2): union, intersection and difference of generated pairs of shapes (integer rectangles and rings, circles, stars; clockwise half of the time) checked against exact references built from the inputs, all areas on one scanline frame; outputs counter-clockwise without overlaps |
| `properties.proptest-regressions` | Seeds that proptest saved from earlier failures; it reruns them first |

| Subfolder | Context |
|---|---|
| `common/` | [CONTEXT.md](common/CONTEXT.md): case set and scanline oracle shared by the tests and the bench |
