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
    #[error("glyph {glyph}, {layer}: identifier {identifier} appears twice")]
    DuplicateIdentifier {
        glyph: Name,
        layer: LayerId,
        identifier: Identifier,
    },
    /// The key is managed elsewhere (`public.objectLibs` by norad, `guidelines` by
    /// [`FontInfo::guidelines`](crate::FontInfo::guidelines)).
    #[error("the key {0:?} is reserved")]
    ReservedKey(String),
    #[error("the glyph table is full")]
    TableFull,
}
