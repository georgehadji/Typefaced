//! Errors raised when a change would break a model invariant.

use crate::{GlyphId, Identifier, LayerId, Name, SourceId};

/// Why a contour's point sequence breaks the UFO rules (the checks norad 0.18.4's
/// outline builder makes when it parses a `.glif`, plus two it enforces by dropping data).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ContourError {
    /// norad drops empty contours on load, so the model never holds one.
    #[error("the contour has no points")]
    Empty,
    #[error("point {index}: a `move` point is allowed only at the start of a contour")]
    UnexpectedMove { index: usize },
    #[error("point {index}: a `line` point cannot follow an off-curve point")]
    PointAfterOffCurve { index: usize },
    #[error("point {index}: a `curve` point follows more than two off-curve points")]
    TooManyOffCurves { index: usize },
    #[error("an open contour ends with off-curve points")]
    TrailingOffCurves,
    /// norad clears `smooth` on off-curve points when it loads them.
    #[error("point {index}: an off-curve point cannot be smooth")]
    SmoothOffCurve { index: usize },
}

/// What holds a set of identified objects. Identifiers are unique within one owner, as
/// norad requires when it reads a `.glif` and when it saves `fontinfo.plist`.
#[derive(Clone, Debug, PartialEq)]
pub enum ObjectOwner {
    /// One `.glif`: contours, points, components, anchors and guidelines.
    GlyphLayer { glyph: Name, layer: LayerId },
    /// The guidelines of one source's `fontinfo.plist`.
    FontInfo(SourceId),
}

impl std::fmt::Display for ObjectOwner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GlyphLayer { glyph, layer } => write!(f, "glyph {glyph}, {layer}"),
            Self::FontInfo(source) => write!(f, "font info of {source}"),
        }
    }
}

/// A change refused at the [`Font`](crate::Font) boundary.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ModelError {
    #[error("invalid name {0:?}")]
    InvalidName(String),
    #[error("invalid identifier {0:?}")]
    InvalidIdentifier(String),
    #[error("colour channel out of range 0..=1: {0}")]
    InvalidColor(f64),
    #[error("the glyph name {0} is already in use")]
    NameInUse(Name),
    #[error("{0} not found")]
    GlyphNotFound(GlyphId),
    #[error("{0} not found")]
    SourceNotFound(SourceId),
    #[error("{0} not found")]
    LayerNotFound(LayerId),
    #[error("{0} is still used (default layer, a glyph or a designspace master)")]
    LayerInUse(LayerId),
    #[error("{0} is used by more than one layer")]
    DuplicateLayerId(LayerId),
    // Not named `source`: thiserror would treat that field as the error's cause.
    #[error("{source_id} has two layers named {name}")]
    DuplicateLayerName { source_id: SourceId, name: Name },
    #[error("{0} has no layers; a UFO always has a default layer")]
    NoLayers(SourceId),
    #[error("{0} already exists")]
    SourceExists(SourceId),
    #[error("glyph {glyph}, {layer}, contour {contour}: {reason}")]
    InvalidContour {
        glyph: Name,
        layer: LayerId,
        contour: usize,
        reason: ContourError,
    },
    #[error("glyph {glyph}, {layer}: code point U+{:04X} appears twice", u32::from(*codepoint))]
    DuplicateCodepoint {
        glyph: Name,
        layer: LayerId,
        codepoint: char,
    },
    #[error("{owner}: identifier {identifier} appears twice")]
    DuplicateIdentifier {
        owner: ObjectOwner,
        identifier: Identifier,
    },
    /// norad stores object libs under their object's identifier (`public.objectLibs`), so an
    /// object with a lib needs one.
    #[error("{owner}: an object has a lib but no identifier")]
    ObjectLibWithoutIdentifier { owner: ObjectOwner },
    #[error("glyph {0} has no layers")]
    NoGlyphLayers(Name),
    /// The key is managed elsewhere (`public.objectLibs` by norad, `guidelines` by
    /// [`FontInfo::guidelines`](crate::FontInfo::guidelines)).
    #[error("the key {0:?} is reserved")]
    ReservedKey(String),
    #[error("the glyph table is full")]
    TableFull,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_name_the_owner_of_the_objects() {
        let owner = ObjectOwner::GlyphLayer {
            glyph: Name::new("A").unwrap(),
            layer: LayerId(2),
        };
        let error = ModelError::ObjectLibWithoutIdentifier { owner };
        assert_eq!(
            error.to_string(),
            "glyph A, LayerId#2: an object has a lib but no identifier"
        );
        let info = ModelError::DuplicateIdentifier {
            owner: ObjectOwner::FontInfo(SourceId(1)),
            identifier: Identifier::new("g").unwrap(),
        };
        assert_eq!(
            info.to_string(),
            "font info of SourceId#1: identifier g appears twice"
        );
    }
}
