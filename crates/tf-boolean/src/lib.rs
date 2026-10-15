//! The `BooleanEngine` port of `tf-geometry` on linesweeper `=0.4.0` (ADR-0008).
//!
//! Every operation is one `linesweeper::binary_op` call under the non-zero fill rule;
//! remove-overlap is the union of a path with an empty path, the call Spike 5 measured
//! and fontc #2070 uses. The output then goes through a debris filter that drops
//! contours below [`DEBRIS_AREA`].

use kurbo::{BezPath, PathEl, Shape};
use linesweeper::{BinaryOp, FillRule};
use tf_geometry::{BooleanEngine, BooleanError};

/// Output contours enclosing less than this, in square font units, are dropped.
///
/// Measured on the regression cases (2026-10-15): the debris that coincident curves leave
/// (`coincident-curves`, `circle-traced-twice`) is 2.4e-9 unit²; the smallest real hole
/// (`tiny-squares-200`) is 4.4e-5 unit², and the smallest in `tiny-circles-200` 2.2e-3.
/// The threshold sits between them, with a margin of over 40x on each side.
pub const DEBRIS_AREA: f64 = 1e-6;

/// The linesweeper engine.
#[derive(Debug, Default, Clone, Copy)]
pub struct Linesweeper;

impl BooleanEngine for Linesweeper {
    fn remove_overlap(&self, path: &BezPath) -> Result<BezPath, BooleanError> {
        run(path, &BezPath::new(), BinaryOp::Union)
    }

    fn union(&self, a: &BezPath, b: &BezPath) -> Result<BezPath, BooleanError> {
        run(a, b, BinaryOp::Union)
    }

    fn intersection(&self, a: &BezPath, b: &BezPath) -> Result<BezPath, BooleanError> {
        run(a, b, BinaryOp::Intersection)
    }

    fn difference(&self, a: &BezPath, b: &BezPath) -> Result<BezPath, BooleanError> {
        run(a, b, BinaryOp::Difference)
    }
}

/// One `binary_op` call with the automatic accuracy (the call Spike 5 measured), then the
/// debris filter.
fn run(a: &BezPath, b: &BezPath, op: BinaryOp) -> Result<BezPath, BooleanError> {
    // linesweeper checks finiteness on the bounding box only, which `f64::min` and `max`
    // can hide (NaN drops out of them); a non-finite point then trips an assertion inside it.
    if !(is_finite(a) && is_finite(b)) {
        return Err(BooleanError::NonFinite);
    }
    let contours = linesweeper::binary_op(a, b, FillRule::NonZero, op).map_err(|e| match e {
        linesweeper::Error::Infinity | linesweeper::Error::NaN => BooleanError::NonFinite,
        linesweeper::Error::NonClosedPath(_) => BooleanError::OpenContour,
    })?;
    Ok(contours
        .contours()
        .filter(|c| c.path.area().abs() >= DEBRIS_AREA)
        .flat_map(|c| c.path.iter())
        .collect())
}

/// Whether every point of `path`, on or off the curve, is finite.
fn is_finite(path: &BezPath) -> bool {
    path.iter().all(|el| match el {
        PathEl::MoveTo(p) | PathEl::LineTo(p) => p.is_finite(),
        PathEl::QuadTo(c, p) => c.is_finite() && p.is_finite(),
        PathEl::CurveTo(c0, c1, p) => c0.is_finite() && c1.is_finite() && p.is_finite(),
        PathEl::ClosePath => true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::{Rect, Shape};

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> BezPath {
        Rect::new(x0, y0, x1, y1).to_path(0.0)
    }

    fn contours(path: &BezPath) -> Vec<f64> {
        let mut areas = Vec::new();
        let mut current = BezPath::new();
        for el in path.iter() {
            if matches!(el, kurbo::PathEl::MoveTo(_)) && !current.is_empty() {
                areas.push(current.area());
                current = BezPath::new();
            }
            current.push(el);
        }
        if !current.is_empty() {
            areas.push(current.area());
        }
        areas
    }

    fn triangle_with(p: kurbo::Point, control: bool) -> BezPath {
        let mut path = BezPath::new();
        path.move_to((0.0, 0.0));
        if control {
            path.quad_to(p, kurbo::Point::new(100.0, 0.0));
        } else {
            path.line_to((100.0, 0.0));
            path.line_to(p);
        }
        path.line_to((50.0, 100.0));
        path.close_path();
        path
    }

    #[test]
    fn remove_overlap_merges_overlapping_squares_into_one_counter_clockwise_contour() {
        let mut two = rect(0.0, 0.0, 100.0, 100.0);
        two.extend(rect(50.0, 0.0, 150.0, 100.0));
        let out = Linesweeper.remove_overlap(&two).unwrap();
        let areas = contours(&out);
        assert_eq!(areas.len(), 1, "{out:?}");
        assert!((areas[0] - 15_000.0).abs() < 1e-6, "{areas:?}");
    }

    #[test]
    fn binary_operations_follow_set_semantics() {
        let a = rect(0.0, 0.0, 100.0, 100.0);
        let b = rect(50.0, 0.0, 150.0, 100.0);
        let area = |p: BezPath| contours(&p).iter().sum::<f64>();
        assert!((area(Linesweeper.union(&a, &b).unwrap()) - 15_000.0).abs() < 1e-6);
        assert!((area(Linesweeper.intersection(&a, &b).unwrap()) - 5_000.0).abs() < 1e-6);
        assert!((area(Linesweeper.difference(&a, &b).unwrap()) - 5_000.0).abs() < 1e-6);
        assert!(
            Linesweeper
                .intersection(&a, &rect(200.0, 0.0, 300.0, 1.0))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn counters_come_back_clockwise() {
        let mut ring = rect(0.0, 0.0, 300.0, 300.0);
        ring.extend(rect(100.0, 100.0, 200.0, 200.0).reverse_subpaths());
        let mut areas = contours(&Linesweeper.remove_overlap(&ring).unwrap());
        areas.sort_by(f64::total_cmp);
        assert_eq!(areas.len(), 2, "{areas:?}");
        assert!((areas[0] + 10_000.0).abs() < 1e-6, "{areas:?}");
        assert!((areas[1] - 90_000.0).abs() < 1e-6, "{areas:?}");
    }

    #[test]
    fn non_finite_coordinates_are_refused_on_and_off_the_curve() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            for control in [false, true] {
                for p in [kurbo::Point::new(bad, 10.0), kurbo::Point::new(10.0, bad)] {
                    let path = triangle_with(p, control);
                    let ok = rect(0.0, 0.0, 1.0, 1.0);
                    assert_eq!(
                        Linesweeper.remove_overlap(&path),
                        Err(BooleanError::NonFinite),
                        "{path:?}"
                    );
                    assert_eq!(Linesweeper.union(&ok, &path), Err(BooleanError::NonFinite));
                    assert_eq!(
                        Linesweeper.difference(&path, &ok),
                        Err(BooleanError::NonFinite)
                    );
                }
            }
        }
    }

    #[test]
    fn open_contours_are_refused() {
        let mut open = BezPath::new();
        open.move_to((0.0, 0.0));
        open.line_to((100.0, 0.0));
        open.line_to((50.0, 100.0));
        assert_eq!(
            Linesweeper.remove_overlap(&open),
            Err(BooleanError::OpenContour)
        );
        assert_eq!(
            Linesweeper.intersection(&rect(0.0, 0.0, 1.0, 1.0), &open),
            Err(BooleanError::OpenContour)
        );
    }

    #[test]
    fn empty_input_gives_empty_output() {
        assert!(
            Linesweeper
                .remove_overlap(&BezPath::new())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn small_real_contours_survive_the_debris_filter() {
        let mut path = rect(0.0, 0.0, 100.0, 100.0);
        // A real but tiny contour, far from the square: well above the threshold.
        path.extend(rect(200.0, 0.0, 200.1, 0.1));
        let areas = contours(&Linesweeper.remove_overlap(&path).unwrap());
        assert_eq!(areas.len(), 2, "{areas:?}");
    }
}
