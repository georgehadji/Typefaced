# crates/tf-geometry/benches/

Up: [tf-geometry/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `hit_test.rs` | Headless timing loop (no harness, no extra dependencies) of `hit_test` and `translate_points` on 5,000-point synthetic glyphs of M0 Step 8: a grid of 50 contours (best case for the contour-box culling) and 2 x 2,500 and 5 x 1,000 concentric contours (worst case); prints mean, p50, p95, p99 and max per case. Run: `cargo bench -p tf-geometry` |
