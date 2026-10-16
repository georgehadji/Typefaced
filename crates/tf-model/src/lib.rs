//! `tf-model`: the domain model of a font project (implementation plan §5.1), on persistent
//! collections (ADR-0003, ADR-0017).
//!
//! - [`Font`] is the aggregate root. Cloning it copies a few pointers; every change goes
//!   through a `Font` method that checks the invariants and leaves the font unchanged on error.
//! - [`GlyphTable`] is an `imbl::Vector<Option<Arc<Glyph>>>` indexed by [`GlyphId`], with a
//!   name index. Deleted glyphs leave tombstones; [`Font::compact`] runs on load only.
//! - A [`Source`] is one UFO's font-level data (M1 deviation 3). A [`Glyph`] holds one
//!   [`GlyphLayer`] (one `.glif`) per [`LayerId`]; every layer belongs to one source.
//! - Model data names glyphs (components, kerning, groups), never IDs.
//! - Pure: no I/O, no serde (M1 deviation 5). `tf-io` maps norad types to these.
//!
//! # Inventory: where every norad 0.18.4 field lives
//!
//! This table is the contract for `tf-io` (M1 Steps 9 and 10). **Typed** fields are ones an M1
//! command or invariant needs; **opaque** ones are carried as ordered plist values
//! ([`LibValue`]) and never reordered. **Shadow** means `tf-io` keeps it in the loaded
//! `norad::Font` of its session and saves from that (gate G10). **None** means nothing keeps
//! it; the G1(d) loss report lists what that drops.
//!
//! | norad 0.18.4 | Home | Kind | Why |
//! |---|---|---|---|
//! | `Font::meta` (`MetaInfo`: creator, format version, minor) | shadow | — | norad rewrites `metainfo.plist` on save (its own creator, UFO 3); no command reads it |
//! | `Font::font_info` (`FontInfo`, ~100 fields) | [`Source::font_info`] → [`FontInfo::fields`] | opaque | `font.setInfo` names fields by UFO key; typed accessors read the few that commands use ([`keys`]) |
//! | `FontInfo::guidelines` (with private identifier and lib) | [`FontInfo::guidelines`] | typed | Each carries an object lib, which norad keeps outside the plist; norad's save refuses duplicate guideline identifiers |
//! | `Font::layers` (`LayerContents`: layer order) | [`Source::layers`] order, default first | typed | Layers are entities (`LayerId`) |
//! | `LayerContents::path_set` (private) | none | — | Derived from layer folder names |
//! | `Layer::name` | [`SourceLayer::name`] | typed | Designspace masters and the UI name layers |
//! | `Layer::path` (private: folder name) | shadow | — | G10: kept so existing folders keep their names |
//! | `Layer::contents` (private: glyph name → `.glif` file name) | shadow | — | G10: kept so existing glyphs keep their file names |
//! | `Layer::path_set` (private) | none | — | Derived from `.glif` file names |
//! | `Layer::color`, `Layer::lib` | [`SourceLayer::color`], [`SourceLayer::lib`] | typed, opaque | `layerinfo.plist` |
//! | `Layer::glyphs` (by name) | [`GlyphTable`]: [`Glyph::layers`] keyed by [`LayerId`] | typed | One glyph entity across all layers and sources |
//! | `Font::lib` | [`Source::lib`] | opaque | Never holds `public.objectLibs` ([`OBJECT_LIBS_KEY`]) |
//! | `Font::groups` | [`Source::groups`] (`imbl::OrdMap`) | typed | `kerning.setGroup`, `glyph.rename` |
//! | `Font::kerning` | [`Source::kerning`] (`imbl::OrdMap<(Name, Name), f64>`) | typed | `kerning.*`, `glyph.rename`. A first side with no pairs (empty inner dict) has no home |
//! | `Font::features` | [`Source::features`] | typed (text) | `features.setCode` |
//! | `Font::data`, `Font::images` (`Store`: path → bytes) | [`Source::data`], [`Source::images`] | opaque bytes | `tf-io` loads every item; a lazy or failed item state and `ufo_root` have no home |
//! | `Glyph::name` | [`Glyph::name`] | typed | Unique in the table |
//! | `Glyph::width`, `height` | [`GlyphLayer::width`], [`GlyphLayer::height`] | typed | `glyph.setWidth`; per layer, as in UFO |
//! | `Glyph::codepoints` | [`GlyphLayer::codepoints`] | typed | `glyph.setCodepoints` writes every layer; [`Font::codepoints`] reads the default layer |
//! | `Glyph::note` | [`GlyphLayer::note`] | typed | Per layer, as in UFO |
//! | `Glyph::guidelines` | [`GlyphLayer::guidelines`] ([`Guideline`]) | typed | Value objects |
//! | `Glyph::anchors` | [`GlyphLayer::anchors`] ([`Anchor`]) | typed | Anchor commands |
//! | `Glyph::components` | [`GlyphLayer::components`] ([`Component`], base by name) | typed | Component commands; dangling bases are [`Warning`]s |
//! | `Glyph::contours` | [`GlyphLayer::contours`] ([`Contour`], [`Point`]) | typed | Outline commands; checked by [`check_contour`] |
//! | `Glyph::image` (`Image`: private file name, colour, transform) | [`GlyphLayer::image`] ([`Image`]) | typed | Value object |
//! | `Glyph::lib` | [`GlyphLayer::lib`] | opaque | Never holds `public.objectLibs` |
//! | `Contour`/`ContourPoint`/`Component`/`Anchor`/`Guideline` private `identifier` and `lib` | the `identifier` and `lib` field of each value object | typed, opaque | norad moves them out of `public.objectLibs` on load and back on save, keyed by identifier. Identifiers are unique per layer (per source for font-info guidelines), and an object with a lib needs an identifier |
//! | `ContourPoint::{x, y, typ, smooth, name}` | [`Point`] | typed | `smooth` is never set on an off-curve point (norad clears it) |
//! | `AffineTransform` | [`Transform`] | typed | Same field names |
//! | `Color` (private channels) | [`Color`] | typed | Channels checked to `0..=1`, as norad does |
//! | `Name`, `Identifier` | [`Name`], [`Identifier`] | typed | Same validity rules, so conversion back never fails |
//! | `.glif` format version | none | — | norad does not keep it (it writes format 2) |
//! | `DesignSpaceDocument` and every nested type | [`Designspace`] and its types | typed | Mirrored field for field, numbers kept as `f32` |
//! | `designspace::Source` | [`Master`] | typed | `filename` verbatim; `layer` resolved to a [`LayerId`] of the master's [`Source`] |
//! | Project files outside the UFOs (`.typefaced` package, `typefaced/` folder) | [`Font::project`] ([`ProjectFiles`]) | opaque bytes | Filled by Step 11 |

mod designspace;
mod error;
mod font;
mod glyph;
mod ids;
mod lib_value;
mod name;
mod source;
mod table;
mod validate;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use designspace::{
    Axis, AxisMapping, AxisMappingEntry, AxisMappings, Condition, ConditionSet, Designspace,
    Dimension, Instance, LocalizedString, Master, Rule, RuleProcessing, Rules, Substitution,
};
pub use error::{ContourError, ModelError, ObjectOwner};
pub use font::{Font, ProjectFiles};
pub use glyph::{
    Anchor, Color, Component, Contour, Glyph, GlyphLayer, Guideline, Image, Line, Point, PointKind,
    Transform,
};
pub use ids::{GlyphId, LayerId, SourceId};
pub use lib_value::{LibDict, LibValue, OBJECT_LIBS_KEY};
pub use name::{Identifier, Name};
pub use source::{FileStore, FontInfo, Groups, Kerning, Source, SourceLayer, keys};
pub use table::GlyphTable;
pub use validate::{Warning, check_contour, check_font_info, check_glyph_layer, check_lib};
