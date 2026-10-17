# crates/tf-boolean/benches/

Up: [tf-boolean/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `ops.rs` | Headless timing loop (no harness) of remove-overlap, intersection and difference (each case against a copy of itself shifted by a third of its size) on the fixture and synthetic cases; prints p50, p95 and max per operation and group, no budget. Run: `cargo bench -p tf-boolean` |
