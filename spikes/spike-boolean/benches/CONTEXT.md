# spikes/spike-boolean/benches/

Up: [spike-boolean/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `engines.rs` | Criterion timing benchmark: each candidate engine removes overlaps over a whole case group (`source-sans`, `fixtures`, `synthetic`), using only the cases every candidate completes. Run: `cargo bench --manifest-path spikes/spike-boolean/Cargo.toml` |
