//! Glyphs and their value objects: points, contours, components, anchors, guidelines, images.
//!
//! Value objects have public fields and compare by value. They are checked when a glyph enters
//! the [`Font`](crate::Font) (see [`validate`](crate::validate)), not when they are built.

use std::collections::BTreeMap;
use std::sync::Arc;

use smallvec::SmallVec;

use crate::{Identifier, LayerId, LibDict, ModelError, Name};

/// UFO point types (glif `type` attribute).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointKind {
    Move,
    Line,
    OffCurve,
    Curve,
    QCurve,
}

/// One contour point. `name`, `identifier` and `lib` are rare; the lib is behind an `Arc` so a
/// layer clone stays cheap.
#[derive(Clone, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub kind: PointKind,
    pub smooth: bool,
    pub name: Option<Name>,
    pub identifier: Option<Identifier>,
    /// The point's object lib (norad: `public.objectLibs` entry of the glyph lib).
    pub lib: Option<Arc<LibDict>>,
}

impl Point {
    /// A plain point: not smooth, no name, identifier or lib.
    pub fn new(x: f64, y: f64, kind: PointKind) -> Self {
        Self {
            x,
            y,
            kind,
            smooth: false,
            name: None,
            identifier: None,
            lib: None,
        }
    }
}

/// A contour. It is open when it starts with a `move` point, closed otherwise.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Contour {
    pub points: Vec<Point>,
    pub identifier: Option<Identifier>,
    pub lib: Option<Arc<LibDict>>,
}

impl Contour {
    /// A contour with no identifier or lib.
    pub fn new(points: Vec<Point>) -> Self {
        Self {
            points,
            ..Self::default()
        }
    }

    /// Whether the contour is closed (it does not start with a `move` point).
    pub fn is_closed(&self) -> bool {
        self.points
            .first()
            .is_none_or(|p| p.kind != PointKind::Move)
    }
}

/// A 2D affine transformation, with norad's field names.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub x_scale: f64,
    pub xy_scale: f64,
    pub yx_scale: f64,
    pub y_scale: f64,
    pub x_offset: f64,
    pub y_offset: f64,
}

impl Default for Transform {
    /// The identity.
    fn default() -> Self {
        Self {
            x_scale: 1.0,
            xy_scale: 0.0,
            yx_scale: 0.0,
            y_scale: 1.0,
            x_offset: 0.0,
            y_offset: 0.0,
        }
    }
}

/// A reference to another glyph, by name. A name that resolves to no glyph is legal in UFO
/// files; [`Font::warnings`](crate::Font::warnings) reports it.
#[derive(Clone, Debug, PartialEq)]
pub struct Component {
    pub base: Name,
    pub transform: Transform,
    pub identifier: Option<Identifier>,
    pub lib: Option<Arc<LibDict>>,
}

/// An RGBA colour; every channel is in `0.0..=1.0`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    red: f64,
    green: f64,
    blue: f64,
    alpha: f64,
}

impl Color {
    /// A colour; fails when a channel is outside `0.0..=1.0` or NaN.
    pub fn new(red: f64, green: f64, blue: f64, alpha: f64) -> Result<Self, ModelError> {
        if let Some(bad) = [red, green, blue, alpha]
            .into_iter()
            .find(|c| !(0.0..=1.0).contains(c))
        {
            return Err(ModelError::InvalidColor(bad));
        }
        Ok(Self {
            red,
            green,
            blue,
            alpha,
        })
    }

    /// `(red, green, blue, alpha)`.
    pub fn channels(&self) -> (f64, f64, f64, f64) {
        (self.red, self.green, self.blue, self.alpha)
    }
}

/// A named position on a glyph layer, for attaching marks.
#[derive(Clone, Debug, PartialEq)]
pub struct Anchor {
    pub x: f64,
    pub y: f64,
    pub name: Option<Name>,
    pub color: Option<Color>,
    pub identifier: Option<Identifier>,
    pub lib: Option<Arc<LibDict>>,
}

/// The infinite line of a guideline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Line {
    Vertical(f64),
    Horizontal(f64),
    /// Through `(x, y)` at `degrees` counter-clockwise from the horizontal.
    Angle {
        x: f64,
        y: f64,
        degrees: f64,
    },
}

/// A guideline of a glyph layer or of a source (`fontinfo.plist`).
#[derive(Clone, Debug, PartialEq)]
pub struct Guideline {
    pub line: Line,
    pub name: Option<Name>,
    pub color: Option<Color>,
    pub identifier: Option<Identifier>,
    pub lib: Option<Arc<LibDict>>,
}

/// A reference to a file in the source's `images/` store.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub file_name: String,
    pub color: Option<Color>,
    pub transform: Transform,
}

/// One `.glif` file: everything a glyph holds in one layer. In UFO every master's `.glif` has
/// its own code points, note, lib, guidelines, image and height, so they live here.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GlyphLayer {
    pub width: f64,
    pub height: f64,
    /// In file order; the first is the primary code point. No duplicates.
    pub codepoints: SmallVec<[char; 1]>,
    pub note: Option<String>,
    pub guidelines: Vec<Guideline>,
    pub anchors: Vec<Anchor>,
    pub components: Vec<Component>,
    pub contours: Vec<Contour>,
    pub image: Option<Image>,
    /// Shared, so editing a point does not copy the lib.
    pub lib: Arc<LibDict>,
}

/// A glyph: its name and its layers. A glyph may be missing from any layer, the default one
/// included (sparse masters, background-only glyphs).
#[derive(Clone, Debug, PartialEq)]
pub struct Glyph {
    pub name: Name,
    pub layers: BTreeMap<LayerId, Arc<GlyphLayer>>,
}

impl Glyph {
    /// A glyph with no layers.
    pub fn new(name: Name) -> Self {
        Self {
            name,
            layers: BTreeMap::new(),
        }
    }

    /// The code points of the glyph in `layer`; empty if the glyph is not in that layer.
    pub fn codepoints(&self, layer: LayerId) -> &[char] {
        self.layers.get(&layer).map_or(&[], |l| &l.codepoints)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(s: &str) -> Name {
        Name::new(s).unwrap()
    }

    #[test]
    fn value_objects_compare_by_value() {
        let a = Point::new(1.0, 2.0, PointKind::Line);
        let mut b = a.clone();
        assert_eq!(a, b);
        b.smooth = true;
        assert_ne!(a, b);

        let anchor = Anchor {
            x: 0.0,
            y: 700.0,
            name: Some(name("top")),
            color: None,
            identifier: None,
            lib: None,
        };
        assert_eq!(anchor.clone(), anchor);

        let guide = Guideline {
            line: Line::Angle {
                x: 1.0,
                y: 2.0,
                degrees: 45.0,
            },
            name: None,
            color: Some(Color::new(1.0, 0.0, 0.0, 0.5).unwrap()),
            identifier: None,
            lib: None,
        };
        assert_eq!(guide.clone(), guide);
        assert_ne!(Line::Vertical(1.0), Line::Horizontal(1.0));

        let component = Component {
            base: name("A"),
            transform: Transform::default(),
            identifier: None,
            lib: None,
        };
        let mut moved = component.clone();
        moved.transform.x_offset = 10.0;
        assert_ne!(component, moved);
    }

    #[test]
    fn transform_default_is_identity() {
        let t = Transform::default();
        assert_eq!((t.x_scale, t.y_scale), (1.0, 1.0));
        assert_eq!(
            (t.xy_scale, t.yx_scale, t.x_offset, t.y_offset),
            (0.0, 0.0, 0.0, 0.0)
        );
    }

    #[test]
    fn colours_must_be_in_range() {
        let c = Color::new(0.0, 0.25, 1.0, 1.0).unwrap();
        assert_eq!(c.channels(), (0.0, 0.25, 1.0, 1.0));
        assert_eq!(
            Color::new(0.0, 1.5, 0.0, 1.0),
            Err(ModelError::InvalidColor(1.5))
        );
        assert!(Color::new(f64::NAN, 0.0, 0.0, 1.0).is_err());
    }

    #[test]
    fn contours_are_closed_unless_they_start_with_a_move() {
        assert!(Contour::new(vec![Point::new(0.0, 0.0, PointKind::Line)]).is_closed());
        assert!(!Contour::new(vec![Point::new(0.0, 0.0, PointKind::Move)]).is_closed());
        assert!(Contour::default().is_closed());
    }

    #[test]
    fn codepoints_are_read_per_layer() {
        let mut glyph = Glyph::new(name("A"));
        let layer = GlyphLayer {
            codepoints: SmallVec::from_slice(&['A', 'a']),
            ..GlyphLayer::default()
        };
        glyph.layers.insert(LayerId(0), Arc::new(layer));
        assert_eq!(glyph.codepoints(LayerId(0)), ['A', 'a']);
        assert!(glyph.codepoints(LayerId(1)).is_empty());
        let image = Image {
            file_name: "a.png".into(),
            color: None,
            transform: Transform::default(),
        };
        assert_eq!(image.clone(), image);
    }
}
