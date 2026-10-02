# tests/fixtures/

Small, hand-made font sources committed to the repository. Up: [tests/CONTEXT.md](../CONTEXT.md).

## min.ufo/

A minimal UFO 3 font, family "Typefaced Test" Regular, 1000 units per em. There is no
`CONTEXT.md` inside it on purpose: it is a UFO package, and extra files would change the
fixture.

| File | What it does |
|---|---|
| `min.ufo/metainfo.plist` | UFO format version 3, creator `com.typefaced.fixtures` |
| `min.ufo/fontinfo.plist` | Family and style names, units per em, ascender, descender, x-height, cap height |
| `min.ufo/layercontents.plist` | Maps the default layer to `glyphs/` |
| `min.ufo/glyphs/contents.plist` | Maps glyph names to `.glif` file names |
| `min.ufo/glyphs/_notdef.glif` | `.notdef`: two contours (box and counter), advance 500 |
| `min.ufo/glyphs/space.glif` | `space`: advance 250, no contours |
| `min.ufo/glyphs/A_.glif` | `A`: two contours, advance 600 |
| `min.ufo/glyphs/O_.glif` | `O`: two curved contours (outer and counter), advance 640 |
| `min.ufo/glyphs/acute.glif` | `acute`: accent, one contour, advance 300 |
| `min.ufo/glyphs/A_acute.glif` | `Aacute`: composite, two components (`A` + `acute`), no contours, advance 600 |
