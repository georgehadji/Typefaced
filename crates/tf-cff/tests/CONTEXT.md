# crates/tf-cff/tests/

Up: [tf-cff/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `read_back.rs` | Builds small tables and reads them back: read-fonts checks the Name INDEX, charset SIDs (standard and custom), widths, FontBBox, FontMatrix and the exact charstring commands; skrifa draws the glyphs from a minimal sfnt with identical points (units per em 1000 and 2048). Also every builder error |
