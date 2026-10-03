# crates/tf-compile/src/

Up: [tf-compile/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `lib.rs` | `compile_to_ttf(path)`: compiles a `.ufo` or `.designspace` to TTF bytes (static or variable) with fontc's default options. `compile_to_otf(path)`: a `.ufo` to OTF (`compile_to_ttf`, then `ttf_to_otf` with the UFO's outlines). `CompileError` covers an unsupported source, a fontc error or panic (caught and turned into an error), and the OTF errors: UFO reading, bad source outlines, duplicate production names, a glyph without a source outline, an unreadable or unconvertible TTF, CFF writer and table errors |
| `otf.rs` | `ttf_to_otf(ttf, outlines)`: builds the `CFF ` table in the TTF's glyph order (names from `post`, PostScript name from `name`), takes a `.notdef` fontc made up from `glyf` (contours reversed), drops `glyf`, `loca`, `cvt `, `fpgm`, `prep`, `hdmx`, `LTSH`, `VDMX`, `gasp`, `DSIG`, writes `maxp` 0.5 and `post` 3.0, recomputes the `head` box, `hmtx` left side bearings and `hhea` extents from the cubic bounds, and copies every other table byte for byte; `FontBuilder` sets `OTTO` and the checksums. Rejects variable fonts, `vmtx`, fonts that already have CFF and a bad `numberOfHMetrics` |
| `source.rs` | `SourceOutlines`: cubic outlines by glyph name. `from_ufo` reads the default layer with norad, decomposes components through their transforms once per glyph (reversing mirrored ones; cycles, deep nesting and outlines over 65,536 elements are errors), leaves out `public.skipExportGlyphs` and names glyphs as fontc does (`public.postscriptNames`, cleaned). Also built from (name, path) pairs |
