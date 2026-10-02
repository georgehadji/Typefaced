# spikes/spike-collections/benches/

Benchmarks at 1,000, 30,000 and 65,535 glyphs (65,535 is the OpenType maximum).
Up: [spike-collections/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `ops.rs` | Criterion timing benchmarks for each candidate: `clone`, `replace_1` (at several positions), `replace_100`, `iterate`, `lookup_1000`. Run: `cargo bench --manifest-path spikes/spike-collections/Cargo.toml --bench ops` |
| `memory.rs` | Memory held by 200 undo snapshots with one single-glyph edit each, measured with a counting global allocator; deterministic. Run: `cargo bench --manifest-path spikes/spike-collections/Cargo.toml --bench memory` |
