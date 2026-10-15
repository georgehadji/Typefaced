//! Invariant checks used at the [`Font`](crate::Font) boundary, and the warnings a valid font
//! can still produce.

use std::collections::HashSet;

use crate::{
    ContourError, GlyphId, GlyphLayer, LayerId, LibDict, ModelError, Name, OBJECT_LIBS_KEY, Point,
    PointKind,
};

/// Checks a contour's point sequence against the UFO rules, as norad 0.18.4 does on load:
/// `move` only first; no `line` after an off-curve; at most two off-curves before a `curve`
/// (any number before a `qcurve`); in a closed contour trailing off-curves wrap around to the
/// start; an open contour cannot end with off-curves. Also refused: empty contours and smooth
/// off-curves, which norad never produces.
pub fn check_contour(points: &[Point]) -> Result<(), ContourError> {
    let first = points.first().ok_or(ContourError::Empty)?;
    let mut off_curves = 0usize;
    for (index, point) in points.iter().enumerate() {
        match point.kind {
            PointKind::Move if index > 0 => return Err(ContourError::UnexpectedMove { index }),
            PointKind::Move => {}
            PointKind::Line if off_curves > 0 => {
                return Err(ContourError::PointAfterOffCurve { index });
            }
            PointKind::Line => {}
            PointKind::OffCurve if point.smooth => {
                return Err(ContourError::SmoothOffCurve { index });
            }
            PointKind::OffCurve => off_curves += 1,
            PointKind::QCurve => off_curves = 0,
            PointKind::Curve if off_curves > 2 => {
                return Err(ContourError::TooManyOffCurves { index });
            }
            PointKind::Curve => off_curves = 0,
        }
    }
    if off_curves == 0 {
        return Ok(());
    }
    if first.kind == PointKind::Move {
        return Err(ContourError::TrailingOffCurves);
    }
    // Closed contour: the trailing off-curves continue at the start. All off-curves is valid.
    for (index, point) in points.iter().enumerate() {
        match point.kind {
            PointKind::OffCurve => off_curves += 1,
            PointKind::QCurve => break,
            PointKind::Curve if off_curves > 2 => {
                return Err(ContourError::TooManyOffCurves { index });
            }
            PointKind::Curve => break,
            // A closed contour has no `move`, so only `line` reaches this arm.
            PointKind::Line | PointKind::Move => {
                return Err(ContourError::PointAfterOffCurve { index });
            }
        }
    }
    Ok(())
}

/// Checks one glyph layer: valid contours, unique code points and identifiers, no reserved key
/// in its lib.
pub fn check_glyph_layer(
    glyph: &Name,
    layer: LayerId,
    data: &GlyphLayer,
) -> Result<(), ModelError> {
    check_lib(&data.lib)?;
    for (contour, c) in data.contours.iter().enumerate() {
        check_contour(&c.points).map_err(|reason| ModelError::InvalidContour {
            glyph: glyph.clone(),
            layer,
            contour,
            reason,
        })?;
    }
    let mut seen = HashSet::new();
    if let Some(&codepoint) = data.codepoints.iter().find(|&&c| !seen.insert(c)) {
        return Err(ModelError::DuplicateCodepoint {
            glyph: glyph.clone(),
            layer,
            codepoint,
        });
    }
    let identifiers = data
        .contours
        .iter()
        .flat_map(|c| std::iter::once(&c.identifier).chain(c.points.iter().map(|p| &p.identifier)))
        .chain(data.components.iter().map(|c| &c.identifier))
        .chain(data.anchors.iter().map(|a| &a.identifier))
        .chain(data.guidelines.iter().map(|g| &g.identifier))
        .flatten();
    let mut seen = HashSet::new();
    if let Some(identifier) = identifiers.into_iter().find(|&i| !seen.insert(i)) {
        return Err(ModelError::DuplicateIdentifier {
            glyph: glyph.clone(),
            layer,
            identifier: identifier.clone(),
        });
    }
    Ok(())
}

/// Refuses `public.objectLibs` in a font or glyph lib (norad refuses to save it).
pub fn check_lib(lib: &LibDict) -> Result<(), ModelError> {
    if lib.contains_key(OBJECT_LIBS_KEY) {
        return Err(ModelError::ReservedKey(OBJECT_LIBS_KEY.to_owned()));
    }
    Ok(())
}

/// Something legal in a UFO that a user probably wants to know about.
#[derive(Clone, Debug, PartialEq)]
pub enum Warning {
    /// A component names a glyph the font does not have.
    DanglingComponent {
        glyph: GlyphId,
        layer: LayerId,
        base: Name,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Anchor, Component, Contour, Guideline, Identifier, LibValue, Line, PointKind, Transform,
    };
    use PointKind::{Curve, Line as L, Move, OffCurve as Off, QCurve};

    fn contour(kinds: &[PointKind]) -> Vec<Point> {
        kinds
            .iter()
            .enumerate()
            .map(|(i, &kind)| Point::new(i as f64, 0.0, kind))
            .collect()
    }

    fn check(kinds: &[PointKind]) -> Result<(), ContourError> {
        check_contour(&contour(kinds))
    }

    #[test]
    fn accepts_valid_sequences() {
        let valid: &[&[PointKind]] = &[
            &[Move],                          // a lone move (open, one point)
            &[Move, L, L],                    // open polyline
            &[L, L, L],                       // closed polygon
            &[L, Off, Off, Curve],            // cubic
            &[L, Off, Curve],                 // one off-curve: quadratic
            &[L, Curve],                      // no off-curve: straight
            &[L, Off, Off, Off, Off, QCurve], // any number before a qcurve
            &[Off, Off, Off, Off],            // all off-curve (TrueType)
            &[Off, Curve, L, Off],            // wraps: two off-curves before the curve
            &[Off, QCurve, L, Off, Off, Off], // wraps into a qcurve: no limit
            &[Move, Off, Off, Curve],         // open cubic
        ];
        for kinds in valid {
            assert_eq!(check(kinds), Ok(()), "{kinds:?}");
        }
    }

    #[test]
    fn refuses_each_norad_error() {
        assert_eq!(check(&[]), Err(ContourError::Empty));
        assert_eq!(
            check(&[L, Move]),
            Err(ContourError::UnexpectedMove { index: 1 })
        );
        assert_eq!(
            check(&[L, Off, L]),
            Err(ContourError::PointAfterOffCurve { index: 2 })
        );
        assert_eq!(
            check(&[L, Off, Off, Off, Curve]),
            Err(ContourError::TooManyOffCurves { index: 4 })
        );
        assert_eq!(check(&[Move, L, Off]), Err(ContourError::TrailingOffCurves));
        // Wrap-around: two trailing plus one leading off-curve before the first curve.
        assert_eq!(
            check(&[Off, Curve, L, Off, Off]),
            Err(ContourError::TooManyOffCurves { index: 1 })
        );
        // Wrap-around into a line.
        assert_eq!(
            check(&[L, Curve, Off]),
            Err(ContourError::PointAfterOffCurve { index: 0 })
        );
        let mut smooth = contour(&[L, Off, Curve]);
        smooth[1].smooth = true;
        assert_eq!(
            check_contour(&smooth),
            Err(ContourError::SmoothOffCurve { index: 1 })
        );
    }

    fn layer_with(f: impl FnOnce(&mut GlyphLayer)) -> GlyphLayer {
        let mut layer = GlyphLayer::default();
        f(&mut layer);
        layer
    }

    fn id(s: &str) -> Option<Identifier> {
        Some(Identifier::new(s).unwrap())
    }

    #[test]
    fn glyph_layers_need_valid_contours_and_unique_codepoints_and_identifiers() {
        let glyph = Name::new("A").unwrap();
        let ok = layer_with(|l| {
            l.codepoints = ['A', 'a'].into_iter().collect();
            l.contours.push(Contour::new(contour(&[L, L, L])));
        });
        assert_eq!(check_glyph_layer(&glyph, LayerId(0), &ok), Ok(()));

        let bad_contour = layer_with(|l| {
            l.contours.push(Contour::new(contour(&[L, L])));
            l.contours.push(Contour::new(Vec::new()));
        });
        assert_eq!(
            check_glyph_layer(&glyph, LayerId(0), &bad_contour),
            Err(ModelError::InvalidContour {
                glyph: glyph.clone(),
                layer: LayerId(0),
                contour: 1,
                reason: ContourError::Empty
            })
        );

        let twice = layer_with(|l| l.codepoints = ['A', 'A'].into_iter().collect());
        assert_eq!(
            check_glyph_layer(&glyph, LayerId(2), &twice),
            Err(ModelError::DuplicateCodepoint {
                glyph: glyph.clone(),
                layer: LayerId(2),
                codepoint: 'A'
            })
        );

        // Identifiers are unique across every object of one layer (norad refuses duplicates
        // when it reads the .glif back).
        let duplicate_ids = layer_with(|l| {
            let mut c = Contour::new(contour(&[L, L, L]));
            c.points[2].identifier = id("x");
            l.contours.push(c);
            l.anchors.push(Anchor {
                x: 0.0,
                y: 0.0,
                name: None,
                color: None,
                identifier: id("x"),
                lib: None,
            });
        });
        assert_eq!(
            check_glyph_layer(&glyph, LayerId(0), &duplicate_ids),
            Err(ModelError::DuplicateIdentifier {
                glyph: glyph.clone(),
                layer: LayerId(0),
                identifier: Identifier::new("x").unwrap()
            })
        );

        let all_kinds_unique = layer_with(|l| {
            let mut c = Contour::new(contour(&[L, L, L]));
            c.identifier = id("c");
            l.contours.push(c);
            l.components.push(Component {
                base: Name::new("B").unwrap(),
                transform: Transform::default(),
                identifier: id("k"),
                lib: None,
            });
            l.guidelines.push(Guideline {
                line: Line::Vertical(0.0),
                name: None,
                color: None,
                identifier: id("g"),
                lib: None,
            });
        });
        assert_eq!(
            check_glyph_layer(&glyph, LayerId(0), &all_kinds_unique),
            Ok(())
        );
    }

    #[test]
    fn libs_cannot_hold_the_object_libs_key() {
        let mut lib = LibDict::new();
        lib.insert("com.example".into(), LibValue::Boolean(true));
        assert_eq!(check_lib(&lib), Ok(()));
        lib.insert(OBJECT_LIBS_KEY.into(), LibValue::Dict(LibDict::new()));
        assert_eq!(
            check_lib(&lib),
            Err(ModelError::ReservedKey(OBJECT_LIBS_KEY.into()))
        );
        let glyph = Name::new("A").unwrap();
        let layer = layer_with(|l| l.lib = lib.clone());
        assert!(check_glyph_layer(&glyph, LayerId(0), &layer).is_err());
    }
}
