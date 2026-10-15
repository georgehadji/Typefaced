//! One UFO's font-level data (M1 deviation 3: per source, not per font): font info, lib,
//! groups, kerning, features, layer list and the `data/` and `images/` stores.
//!
//! Every field is cheap to clone (`Arc` or `imbl`), so editing one source through
//! `Arc::make_mut` copies pointers, not data.

use std::collections::BTreeMap;
use std::sync::Arc;

use imbl::OrdMap;

use crate::{Color, Guideline, LayerId, LibDict, LibValue, Name};

/// Kerning of one source: `(first, second) → value`, where each side is a glyph or group
/// name. `OrdMap` iterates in the order norad writes `kerning.plist` (by first side, then by
/// second), so saving needs no sort. Chosen by `benches/kerning.rs` (ADR-0017 amendment).
pub type Kerning = OrdMap<(Name, Name), f64>;

/// Groups of one source: group name → member glyph names, in file order.
pub type Groups = OrdMap<Name, Vec<Name>>;

/// `fontinfo.plist`, carried opaque: the UFO key → value, except the guidelines.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FontInfo {
    /// Every key norad's `FontInfo` holds, except `guidelines`. Font-data commands name fields
    /// by these keys (`font.setInfo`).
    pub fields: LibDict,
    /// `guidelines`, typed because each guideline carries an object lib (norad keeps it apart
    /// from the plist, in `lib.plist`'s `public.objectLibs`).
    pub guidelines: Option<Vec<Guideline>>,
}

/// `fontinfo.plist` keys that M1 commands read or write.
pub mod keys {
    pub const FAMILY_NAME: &str = "familyName";
    pub const STYLE_NAME: &str = "styleName";
    pub const UNITS_PER_EM: &str = "unitsPerEm";
    pub const ASCENDER: &str = "ascender";
    pub const DESCENDER: &str = "descender";
    pub const X_HEIGHT: &str = "xHeight";
    pub const CAP_HEIGHT: &str = "capHeight";
    pub const ITALIC_ANGLE: &str = "italicAngle";
    /// Held in [`FontInfo::guidelines`](super::FontInfo::guidelines), never in `fields`.
    pub const GUIDELINES: &str = "guidelines";
}

impl FontInfo {
    /// A numeric field (`Integer` or `Real`).
    pub fn number(&self, key: &str) -> Option<f64> {
        self.fields.get(key).and_then(LibValue::as_f64)
    }

    /// A string field.
    pub fn string(&self, key: &str) -> Option<&str> {
        self.fields.get(key).and_then(LibValue::as_str)
    }

    pub fn family_name(&self) -> Option<&str> {
        self.string(keys::FAMILY_NAME)
    }

    pub fn style_name(&self) -> Option<&str> {
        self.string(keys::STYLE_NAME)
    }

    pub fn units_per_em(&self) -> Option<f64> {
        self.number(keys::UNITS_PER_EM)
    }

    pub fn ascender(&self) -> Option<f64> {
        self.number(keys::ASCENDER)
    }

    pub fn descender(&self) -> Option<f64> {
        self.number(keys::DESCENDER)
    }

    pub fn x_height(&self) -> Option<f64> {
        self.number(keys::X_HEIGHT)
    }

    pub fn cap_height(&self) -> Option<f64> {
        self.number(keys::CAP_HEIGHT)
    }

    pub fn italic_angle(&self) -> Option<f64> {
        self.number(keys::ITALIC_ANGLE)
    }
}

/// One UFO layer (`layercontents.plist` entry plus its `layerinfo.plist`). The folder name
/// is not here: `tf-io` keeps it in its shadow norad font (gate G10).
#[derive(Clone, Debug, PartialEq)]
pub struct SourceLayer {
    pub id: LayerId,
    pub name: Name,
    pub color: Option<Color>,
    pub lib: LibDict,
}

impl SourceLayer {
    pub fn new(id: LayerId, name: Name) -> Self {
        Self {
            id,
            name,
            color: None,
            lib: LibDict::new(),
        }
    }
}

/// A file store (`data/` or `images/`): path inside the store, `/`-separated → bytes.
pub type FileStore = Arc<BTreeMap<String, Arc<[u8]>>>;

/// One UFO's font-level data.
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    pub font_info: Arc<FontInfo>,
    pub lib: Arc<LibDict>,
    pub groups: Groups,
    pub kerning: Kerning,
    /// `features.fea`, verbatim.
    pub features: Arc<str>,
    /// In `layercontents.plist` order; the first is the default layer. Never empty.
    pub layers: Vec<SourceLayer>,
    pub data: FileStore,
    pub images: FileStore,
}

impl Source {
    /// An empty source with one layer, its default.
    pub fn new(default_layer: SourceLayer) -> Self {
        Self {
            font_info: Arc::default(),
            lib: Arc::default(),
            groups: Groups::new(),
            kerning: Kerning::new(),
            features: Arc::from(""),
            layers: vec![default_layer],
            data: FileStore::default(),
            images: FileStore::default(),
        }
    }

    pub fn layer(&self, id: LayerId) -> Option<&SourceLayer> {
        self.layers.iter().find(|layer| layer.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Line;

    fn name(s: &str) -> Name {
        Name::new(s).unwrap()
    }

    #[test]
    fn font_info_reads_typed_fields() {
        let mut info = FontInfo::default();
        info.fields
            .insert(keys::FAMILY_NAME.into(), LibValue::String("Test".into()));
        info.fields
            .insert(keys::STYLE_NAME.into(), LibValue::String("Bold".into()));
        info.fields
            .insert(keys::UNITS_PER_EM.into(), LibValue::Integer(1000));
        info.fields
            .insert(keys::ASCENDER.into(), LibValue::Real(750.5));
        info.fields
            .insert(keys::DESCENDER.into(), LibValue::Integer(-250));
        info.fields
            .insert(keys::X_HEIGHT.into(), LibValue::Integer(500));
        info.fields
            .insert(keys::CAP_HEIGHT.into(), LibValue::Integer(700));
        info.fields
            .insert(keys::ITALIC_ANGLE.into(), LibValue::Real(-12.0));
        assert_eq!(info.family_name(), Some("Test"));
        assert_eq!(info.style_name(), Some("Bold"));
        assert_eq!(info.units_per_em(), Some(1000.0));
        assert_eq!(info.ascender(), Some(750.5));
        assert_eq!(info.descender(), Some(-250.0));
        assert_eq!(info.x_height(), Some(500.0));
        assert_eq!(info.cap_height(), Some(700.0));
        assert_eq!(info.italic_angle(), Some(-12.0));
        // Wrong type or missing: no value.
        assert_eq!(info.string(keys::UNITS_PER_EM), None);
        assert_eq!(info.number("openTypeOS2WinAscent"), None);
    }

    #[test]
    fn kerning_iterates_like_the_norad_file() {
        let mut kerning = Kerning::new();
        kerning.insert((name("public.kern1.O"), name("A")), -10.0);
        kerning.insert((name("A"), name("V")), -80.0);
        kerning.insert((name("A"), name("T")), -60.0);
        let order: Vec<(&str, &str)> = kerning
            .keys()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect();
        assert_eq!(order, [("A", "T"), ("A", "V"), ("public.kern1.O", "A")]);
    }

    #[test]
    fn a_new_source_has_one_default_layer() {
        let mut source = Source::new(SourceLayer::new(LayerId(4), name("public.default")));
        assert_eq!(source.layers.len(), 1);
        assert_eq!(
            source.layer(LayerId(4)).unwrap().name.as_str(),
            "public.default"
        );
        assert!(source.layer(LayerId(5)).is_none());
        Arc::make_mut(&mut source.font_info).guidelines = Some(vec![Guideline {
            line: Line::Horizontal(500.0),
            name: None,
            color: None,
            identifier: None,
            lib: None,
        }]);
        assert_ne!(
            source,
            Source::new(SourceLayer::new(LayerId(4), name("public.default")))
        );
    }
}
