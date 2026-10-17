# crates/tf-geometry/src/

Up: [tf-geometry/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `boolean.rs` | The `BooleanEngine` port (remove overlap, union, intersection, difference on `kurbo::BezPath`, non-zero fill rule) and its `BooleanError` (non-exhaustive: non-finite, open contour, out of range). No engine here: the linesweeper adapter is the `tf-boolean` crate (ADR-0008), so the kernel keeps compiling to WASM |
| `lib.rs` | Re-exports the boolean port. The packed outline layout (`coords`, `flags`, `contour_ends`) and the point `flags`; `hit_test`, which finds the nearest point or segment within a radius and returns a `Hit`; `translate_points`, which returns a copy with selected points moved. Segments are visited without allocating, and contours whose bounding box is out of reach are skipped. Tests include property tests |
