//! Glyph outlines as CFF stores them (rounded, closed subpaths) and their Type 2
//! charstrings (TN #5177): `rmoveto`, `rlineto`, `rrcurveto` and `endchar`, no hints.

use kurbo::{BezPath, PathEl, Point, Shape};

use crate::encode::charstring_int;
use crate::{Bounds, OutlineError};

const RLINETO: u8 = 5;
const RRCURVETO: u8 = 8;
const ENDCHAR: u8 = 14;
const RMOVETO: u8 = 21;

type IntPoint = (i64, i64);

#[derive(Debug, Clone, Copy, PartialEq)]
enum Segment {
    Line(IntPoint),
    Curve(IntPoint, IntPoint, IntPoint),
}

/// A subpath with rounded points. CFF closes every subpath: the next `rmoveto` or the
/// `endchar` draws a line back to `start` when the last point is elsewhere.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Subpath {
    start: IntPoint,
    segments: Vec<Segment>,
}

/// OpenType rounding, `floor(v + 0.5)`, as fontTools' `otRound` and fontc's `ot_round`.
fn ot_round(v: f64) -> Result<i64, OutlineError> {
    if !v.is_finite() {
        return Err(OutlineError::NonFinite);
    }
    let rounded = (v + 0.5).floor();
    if rounded < f64::from(i32::MIN) || rounded > f64::from(i32::MAX) {
        // Saturating cast: only for the error message.
        return Err(OutlineError::OutOfRange(rounded as i64));
    }
    Ok(rounded as i64)
}

fn round(p: Point) -> Result<IntPoint, OutlineError> {
    Ok((ot_round(p.x)?, ot_round(p.y)?))
}

/// Splits `path` into rounded subpaths. Rounding the points (not the deltas between
/// them) keeps every point within half a unit of the source. A final line back to the
/// start is dropped (the implicit close draws it), and so are subpaths left without
/// segments, which draw nothing.
pub(crate) fn subpaths(path: &BezPath) -> Result<Vec<Subpath>, OutlineError> {
    let mut done = Vec::new();
    let mut current: Option<Subpath> = None;
    // As in kurbo, a segment right after `ClosePath` starts at the closed subpath's start.
    let mut last_start = None;
    for element in path.elements() {
        let segment = match *element {
            PathEl::MoveTo(p) => {
                finish(&mut done, current.take());
                current = Some(Subpath {
                    start: round(p)?,
                    segments: Vec::new(),
                });
                continue;
            }
            PathEl::ClosePath => {
                last_start = current.as_ref().map(|s| s.start).or(last_start);
                finish(&mut done, current.take());
                continue;
            }
            PathEl::LineTo(p) => Segment::Line(round(p)?),
            PathEl::CurveTo(a, b, c) => Segment::Curve(round(a)?, round(b)?, round(c)?),
            PathEl::QuadTo(..) => return Err(OutlineError::Quadratic),
        };
        let subpath = match current.as_mut() {
            Some(subpath) => subpath,
            None => current.insert(Subpath {
                start: last_start.ok_or(OutlineError::MissingMove)?,
                segments: Vec::new(),
            }),
        };
        subpath.segments.push(segment);
    }
    finish(&mut done, current);
    Ok(done)
}

fn finish(done: &mut Vec<Subpath>, subpath: Option<Subpath>) {
    let Some(mut subpath) = subpath else {
        return;
    };
    if subpath.segments.last() == Some(&Segment::Line(subpath.start)) {
        subpath.segments.pop();
    }
    if !subpath.segments.is_empty() {
        done.push(subpath);
    }
}

/// The subpaths as a `BezPath`, each ending with `ClosePath`.
pub(crate) fn to_bez_path(subpaths: &[Subpath]) -> BezPath {
    let point = |(x, y): IntPoint| Point::new(x as f64, y as f64);
    let mut path = BezPath::new();
    for subpath in subpaths {
        path.move_to(point(subpath.start));
        for segment in &subpath.segments {
            match *segment {
                Segment::Line(p) => path.line_to(point(p)),
                Segment::Curve(a, b, c) => path.curve_to(point(a), point(b), point(c)),
            }
        }
        path.close_path();
    }
    path
}

/// Extrema closer than this to an integer are taken as that integer: kurbo can find a
/// root at `t = 1 − ε` where the tangent is vertical at an end point, and evaluate the
/// curve a hair outside its true extent, which floor or ceiling would turn into a
/// whole unit.
const SNAP: f64 = 1e-6;

/// The exact bounds of the subpaths, rounded outwards; `None` if there are none.
pub(crate) fn bounds(subpaths: &[Subpath]) -> Option<Bounds> {
    if subpaths.is_empty() {
        return None;
    }
    let rect = to_bez_path(subpaths).bounding_box();
    let outward = |v: f64, round: fn(f64) -> f64| {
        let nearest = v.round();
        // In i32 range: the points were checked when rounded, and a cubic stays
        // inside the hull of its control points.
        (if (v - nearest).abs() < SNAP {
            nearest
        } else {
            round(v)
        }) as i32
    };
    Some(Bounds {
        x_min: outward(rect.x0, f64::floor),
        y_min: outward(rect.y0, f64::floor),
        x_max: outward(rect.x1, f64::ceil),
        y_max: outward(rect.y1, f64::ceil),
    })
}

/// The Type 2 charstring: the width delta (if any) first, then one operator per
/// segment, relative to the current point, then `endchar`.
pub(crate) fn charstring(
    subpaths: &[Subpath],
    width: Option<i64>,
) -> Result<Vec<u8>, OutlineError> {
    let mut out = Vec::new();
    if let Some(width) = width {
        number(&mut out, width)?;
    }
    let mut current = (0, 0);
    for subpath in subpaths {
        delta(&mut out, current, subpath.start)?;
        out.push(RMOVETO);
        current = subpath.start;
        for segment in &subpath.segments {
            match *segment {
                Segment::Line(p) => {
                    delta(&mut out, current, p)?;
                    out.push(RLINETO);
                    current = p;
                }
                Segment::Curve(a, b, c) => {
                    delta(&mut out, current, a)?;
                    delta(&mut out, a, b)?;
                    delta(&mut out, b, c)?;
                    out.push(RRCURVETO);
                    current = c;
                }
            }
        }
    }
    out.push(ENDCHAR);
    Ok(out)
}

fn number(out: &mut Vec<u8>, value: i64) -> Result<(), OutlineError> {
    charstring_int(out, value).map_err(OutlineError::OutOfRange)
}

fn delta(out: &mut Vec<u8>, from: IntPoint, to: IntPoint) -> Result<(), OutlineError> {
    number(out, to.0 - from.0)?;
    number(out, to.1 - from.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounding_is_half_up() {
        let rounded: Vec<i64> = [-1.5, -0.5, -0.4, 0.49, 0.5, 2.5]
            .into_iter()
            .map(|v| ot_round(v).unwrap())
            .collect();
        assert_eq!(rounded, [-1, 0, 0, 0, 1, 3]);
    }

    #[test]
    fn coordinates_beyond_32_bits_are_out_of_range() {
        assert_eq!(ot_round(3e9), Err(OutlineError::OutOfRange(3_000_000_000)));
        assert_eq!(ot_round(f64::INFINITY), Err(OutlineError::NonFinite));
    }

    #[test]
    fn a_square_encodes_as_moveto_lineto_endchar() {
        let mut path = BezPath::new();
        path.move_to((10.0, 20.0));
        path.line_to((110.0, 20.0));
        path.line_to((110.0, 120.0));
        path.close_path();

        let bytes = charstring(&subpaths(&path).unwrap(), Some(-5)).unwrap();

        // -5 | 10 20 rmoveto | 100 0 rlineto | 0 100 rlineto | endchar
        assert_eq!(bytes, [134, 149, 159, 21, 239, 139, 5, 139, 239, 5, 14]);
    }

    #[test]
    fn a_curve_encodes_three_deltas_and_the_next_subpath_moves_from_its_end() {
        let mut path = BezPath::new();
        path.move_to((0.0, 0.0));
        path.curve_to((0.0, 10.0), (10.0, 20.0), (20.0, 20.0));
        path.move_to((30.0, 20.0));
        path.line_to((30.0, 0.0));

        let bytes = charstring(&subpaths(&path).unwrap(), None).unwrap();

        // 0 0 rmoveto | 0 10 10 10 10 0 rrcurveto | 10 0 rmoveto | 0 -20 rlineto | endchar
        assert_eq!(
            bytes,
            [
                139, 139, 21, 139, 149, 149, 149, 149, 139, 8, 149, 139, 21, 139, 119, 5, 14
            ]
        );
    }
}
