# crates/tf-model/benches/

Plain timing programs (no harness, no criterion; M1 deviation 10). Each prints p50, p95,
p99 and max per case and exits with status 1 when its budget fails. CI runs both with
`cargo bench -p tf-model --locked`. Up: [tf-model/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `glyph_table.rs` | Clones a font and replaces one 100-point glyph (`Font::replace_glyph`, checks included) at the first, middle and last ID of 1,000, 30,000 and 65,535-glyph fonts, 1,000 samples each. Budget: middle replace at 30,000 glyphs, p95 < 50 µs (ADR-0017). Also prints, ungated, a middle replace that renames the glyph |
| `kerning.rs` | 50,000 kerning pairs: set one pair (after a clone), look one up, iterate all, for `imbl::OrdMap` (the model's `Kerning`) and `imbl::HashMap`. Also the reducer path through a `Font` (clone, copy-on-write the source, set an existing or a new pair, `replace_source`). Budget: map set and lookup and both `Font` paths, p95 < 50 µs |
