# crates/tf-cff/src/

Specs: Adobe TN #5176 (CFF) and TN #5177 (Type 2 charstrings). One font per table, no
subroutines, no hints. Up: [tf-cff/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `lib.rs` | `CffBuilder::new(font_name, units_per_em).glyph(name, advance, &path)…build()`: checks names (font name, `.notdef` first, unique printable glyph names), assigns string IDs (standard SIDs, custom names from 391), picks the most common advance as `defaultWidthX` and `nominalWidthX`, and lays out Header, Name INDEX, Top DICT INDEX (FontBBox, FontMatrix when units per em ≠ 1000, 5-byte offsets), String INDEX, empty Global Subr INDEX, charset format 0, CharStrings INDEX, Private DICT. `bounds(path)` returns a glyph's integer `Bounds` as the font states them, for the caller's `head`, `hmtx` and `hhea`. `CffError`, `OutlineError` |
| `outline.rs` | Rounds points with `floor(v + 0.5)`, splits a `BezPath` into closed subpaths (drops a final line back to the start and subpaths without segments; rejects quadratic segments), computes their bounds (exact extrema rounded outwards, float noise within 1e-6 of an integer snapped), and encodes the Type 2 charstring (`rmoveto`, `rlineto`, `rrcurveto`, `endchar`, width delta first) |
| `encode.rs` | DICT integers (shortest form, or fixed 5-byte for offsets), DICT reals (nibbles), charstring integers (1, 2 or 3 bytes; 16-bit range), and INDEX with the smallest offset size |
