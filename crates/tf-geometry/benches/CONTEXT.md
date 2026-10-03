# crates/tf-geometry/benches/

Up: [tf-geometry/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `hit_test.rs` | Headless timing loop (no harness, no extra dependencies) of `hit_test` and `translate_points` on the 5,000-point synthetic glyph of M0 Step 8; prints mean, p50, p95, p99 and max per case. Run: `cargo bench -p tf-geometry` |
