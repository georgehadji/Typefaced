# crates/tf-engine/src/reduce/

Reducers: pure functions `(&DocState, &payload) -> Result<Reduced, CommandError>`. One
file per command family, so the family steps edit different files. A reducer that is not
written yet is a stub from `not_implemented!` that fails with `E_NOT_IMPLEMENTED`. Each
family file also holds its `IMPLEMENTED` list (read by the dispatch test), its proptest
generators (`testing`) and the DTO conversions only that family needs; shared conversions
live in `../map.rs`. Up: [src/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `mod.rs` | `reduce`: the dispatch over every catalog command; `Reduced`, `Touched`, `Event`; the `not_implemented!` stub macro; the shared generator list and pickers (`testing`) and the unit-test font (`fixture`) |
| `structure.rs` | Glyph structure: `glyph.create`, `glyph.delete` (refuses glyphs used as components, `E_GLYPH_IN_USE`), `glyph.setWidth`, `glyph.rename`, `glyph.duplicate`, `glyph.setCodepoints` |
| `font.rs` | Font data per source: `font.setInfo`, `font.setVerticalMetrics`, `kerning.*`, `features.setCode` |
| `outline.rs` | Outlines: `glyph.setOutline`, `movePoints`, `insertPoint`, `deletePoints`, `toggleSmooth`, `transform` |
| `parts.rs` | Components and anchors: `glyph.addComponent`, `removeComponent`, `decompose`, `addAnchor`, `moveAnchor`, `removeAnchor` |
