# spikes/spike-collections/src/

Up: [spike-collections/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `lib.rs` | The `GlyphTable` trait and four implementations: `Naive` (`Arc<Vec<Arc<Glyph>>>`), `Chunked` (in-house, 64 glyphs per chunk), `ImblVector` (RRB tree) and `ImblOrdMap` (B+ tree, sparse IDs). Also the synthetic `Glyph`/`Point` model and helpers that build glyphs and edits for the benchmarks |
