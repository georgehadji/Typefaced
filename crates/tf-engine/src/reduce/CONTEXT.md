# crates/tf-engine/src/reduce/

Reducers: pure functions `(&DocState, &payload) -> Result<Reduced, CommandError>`. One
file per command family, so M1 Steps 5 and 6 edit different files. An arm whose reducer
is not written yet fails with `E_NOT_IMPLEMENTED` from a stub in its family file. Each
family file also holds the proptest generators of its commands (`testing`).
Up: [src/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `mod.rs` | `reduce`: the dispatch over every catalog command; `Reduced`, `Touched`, `Event`; the `not_implemented!` stub macro; the shared generator list (`testing::command`) and the unit-test font (`fixture`) |
| `structure.rs` | Glyph structure: `glyph.create`, `glyph.delete` (refuses glyphs used as components, `E_GLYPH_IN_USE`), `glyph.setWidth`; stubs for `glyph.rename`, `glyph.duplicate`, `glyph.setCodepoints` (Step 5) |
| `font.rs` | Font data per source: `font.*`, `kerning.*`, `features.setCode` (Step 5; stubs) |
| `outline.rs` | Outlines: `glyph.setOutline`, `movePoints`, `insertPoint`, `deletePoints`, `toggleSmooth`, `transform` (Step 6; stubs) |
| `parts.rs` | Components and anchors: `glyph.addComponent`, `removeComponent`, `decompose`, `addAnchor`, `moveAnchor`, `removeAnchor` (Step 6; stubs) |
