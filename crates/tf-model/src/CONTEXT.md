# crates/tf-model/src/

Up: [tf-model/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `lib.rs` | Crate docs with the norad 0.18.4 inventory table (typed, opaque, G10 shadow or no home for every field); module list and re-exports |
| `font.rs` | `Font`, the aggregate root: sources, glyph table, default layer, optional designspace, `ProjectFiles`. Every change method checks the invariants first and leaves the font unchanged on error; `compact` (load only) and `warnings` (dangling components) |
| `table.rs` | `GlyphTable`: `imbl::Vector<Option<Arc<Glyph>>>` indexed by `GlyphId` with tombstones, plus an `imbl::OrdMap` name index; insert, replace (rename), remove, `compact`. Model-based property test |
| `glyph.rs` | `Glyph` (name + one `GlyphLayer` per `LayerId`) and the value objects: `Point`, `PointKind`, `Contour`, `Component`, `Transform`, `Anchor`, `Guideline`, `Line`, `Image`, `Color` |
| `source.rs` | `Source`: one UFO's font-level data (font info, lib, groups, kerning, features, layers, `data/` and `images/` stores); `FontInfo` with typed accessors and its `keys`; `SourceLayer`; the `Kerning` and `Groups` maps |
| `designspace.rs` | `Designspace` and its types, mirroring norad's designspace document; `Master` points a `<source>` at a model `Source` and optional layer |
| `validate.rs` | `check_contour` (the UFO point-sequence rules norad applies on load), `check_glyph_layer`, `check_lib`, and `Warning` |
| `error.rs` | `ModelError` and `ContourError` |
| `ids.rs` | `GlyphId`, `SourceId`, `LayerId`: `u32` newtypes |
| `name.rs` | `Name` and `Identifier`, validated with norad's rules |
| `lib_value.rs` | `LibValue` and `LibDict`: opaque, ordered plist values; `OBJECT_LIBS_KEY` |
| `testing.rs` | Proptest generators (`name`, `contour`, `glyph_layer`, `font`) behind the `testing` feature (and in this crate's own tests), with their property tests |
