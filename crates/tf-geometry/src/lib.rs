//! Geometry kernel on packed outlines (implementation plan §5.2, §7.2).
//!
//! A packed outline is three parallel arrays, the same layout the UI holds in typed arrays:
//! - `coords`: interleaved `[x0, y0, x1, y1, …]` in font units;
//! - `flags`: one byte per point, see [`flags`];
//! - `contour_ends`: the index of the last point of each contour (inclusive, like
//!   TrueType's `endPtsOfContours`). Contours are closed.
//!
//! Every function here is pure, never panics on malformed input, and compiles to
//! `wasm32-unknown-unknown` (the `tf-wasm` facade).

use kurbo::{CubicBez, Line, ParamCurveNearest, PathSeg, Point, QuadBez};

/// Point flags: the low two bits are the point type, bit 2 marks a smooth point.
pub mod flags {
    /// An off-curve control point.
    pub const OFF_CURVE: u8 = 0;
    /// An on-curve point that ends a straight segment.
    pub const LINE: u8 = 1;
    /// An on-curve point that ends a cubic segment (two off-curve points before it).
    pub const CURVE: u8 = 2;
    /// An on-curve point that ends a quadratic segment (one off-curve point before it).
    pub const QCURVE: u8 = 3;
    /// Mask for the point type.
    pub const TYPE_MASK: u8 = 0b11;
    /// The point is smooth (its handles stay collinear).
    pub const SMOOTH: u8 = 0b100;
}

/// Accuracy passed to kurbo's nearest-point search, in font units.
const NEAREST_ACCURACY: f64 = 1e-9;

/// Largest coordinate magnitude, in font units, for which segments are hit-tested.
/// Far beyond any real design space; it keeps kurbo's curve solver away from overflow.
pub const MAX_COORD: f64 = 1e9;

/// What [`hit_test`] found.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Hit {
    /// A point (on- or off-curve) by its index.
    Point { index: usize, distance: f64 },
    /// A segment of contour `contour`, from on-curve point `start` to on-curve point `end`;
    /// `t` is the curve parameter of the nearest position.
    Segment {
        contour: usize,
        start: usize,
        end: usize,
        t: f64,
        distance: f64,
    },
}

/// One segment of a packed outline, with the indices of its on-curve end points.
#[derive(Debug, Clone, Copy)]
struct Segment {
    contour: usize,
    start: usize,
    end: usize,
    seg: PathSeg,
}

/// Finds what is under `(x, y)` within `radius`: the nearest point if any point is
/// within the radius, otherwise the nearest segment, across all contours.
///
/// Returns `None` when nothing is within the radius, or when `radius` is negative or NaN.
/// Malformed parts of the outline are skipped: points beyond the shorter of `coords`
/// and `flags`, and contours whose end is out of range or before their start. Segments
/// with a coordinate that is not finite or beyond ±[`MAX_COORD`] cannot be hit.
#[must_use]
pub fn hit_test(
    coords: &[f64],
    flags: &[u8],
    contour_ends: &[u32],
    x: f64,
    y: f64,
    radius: f64,
) -> Option<Hit> {
    // A negative radius would square to a positive limit, so reject it here.
    if radius.is_nan() || radius < 0.0 {
        return None;
    }
    let p = Point::new(x, y);
    nearest_point(coords, flags, p, radius)
        .or_else(|| nearest_segment(coords, flags, contour_ends, p, radius))
}

/// Returns a copy of `coords` in which every point listed in `selection` is moved by
/// `(dx, dy)` exactly once. Duplicate and out-of-range indices are ignored.
#[must_use]
pub fn translate_points(coords: &[f64], selection: &[u32], dx: f64, dy: f64) -> Vec<f64> {
    let mut moved = coords.to_vec();
    let mut done = vec![false; coords.len() / 2];
    for &index in selection {
        let i = index as usize;
        if let Some(seen) = done.get_mut(i).filter(|seen| !**seen) {
            *seen = true;
            moved[2 * i] += dx;
            moved[2 * i + 1] += dy;
        }
    }
    moved
}

fn point_count(coords: &[f64], flags: &[u8]) -> usize {
    (coords.len() / 2).min(flags.len())
}

fn point_at(coords: &[f64], i: usize) -> Point {
    Point::new(coords[2 * i], coords[2 * i + 1])
}

fn nearest_point(coords: &[f64], flags: &[u8], p: Point, radius: f64) -> Option<Hit> {
    let limit = radius * radius;
    let mut best: Option<(usize, f64)> = None;
    for i in 0..point_count(coords, flags) {
        let d2 = (point_at(coords, i) - p).hypot2();
        // `<=` keeps points exactly on the radius; NaN distances never compare true, and
        // an infinite distance (an infinite coordinate) is rejected even for an infinite radius.
        if d2 <= limit && d2.is_finite() && best.is_none_or(|(_, b)| d2 < b) {
            best = Some((i, d2));
        }
    }
    best.map(|(index, d2)| Hit::Point {
        index,
        distance: d2.sqrt(),
    })
}

fn nearest_segment(
    coords: &[f64],
    flags: &[u8],
    contour_ends: &[u32],
    p: Point,
    radius: f64,
) -> Option<Hit> {
    let limit = radius * radius;
    let mut best: Option<(Segment, f64, f64)> = None;
    let near_contour = |first, last| contour_is_near(coords, first, last, p, radius);
    for_each_segment(coords, flags, contour_ends, near_contour, |s| {
        if !within_limits(&s.seg) || !near_control_box(&s.seg, p, radius) {
            return;
        }
        let found = s.seg.nearest(p, NEAREST_ACCURACY);
        if found.distance_sq <= limit && best.is_none_or(|(_, _, b)| found.distance_sq < b) {
            best = Some((s, found.t, found.distance_sq));
        }
    });
    best.map(|(s, t, d2)| Hit::Segment {
        contour: s.contour,
        start: s.start,
        end: s.end,
        t,
        distance: d2.sqrt(),
    })
}

/// Whether every control point of the segment is finite and within ±[`MAX_COORD`].
fn within_limits(seg: &PathSeg) -> bool {
    let ok = |q: &Point| q.x.abs() <= MAX_COORD && q.y.abs() <= MAX_COORD;
    match seg {
        PathSeg::Line(l) => ok(&l.p0) && ok(&l.p1),
        PathSeg::Quad(q) => ok(&q.p0) && ok(&q.p1) && ok(&q.p2),
        PathSeg::Cubic(c) => ok(&c.p0) && ok(&c.p1) && ok(&c.p2) && ok(&c.p3),
    }
}

/// Whether `p` lies inside the segment's control-point box grown by `radius`. The curve
/// lies inside its control polygon's hull, so segments failing this cannot be hit;
/// skipping them keeps the costly nearest-point solve off most segments.
fn near_control_box(seg: &PathSeg, p: Point, radius: f64) -> bool {
    let points: &[Point] = match seg {
        PathSeg::Line(l) => &[l.p0, l.p1],
        PathSeg::Quad(q) => &[q.p0, q.p1, q.p2],
        PathSeg::Cubic(c) => &[c.p0, c.p1, c.p2, c.p3],
    };
    let (mut x0, mut y0, mut x1, mut y1) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for q in points {
        x0 = x0.min(q.x);
        y0 = y0.min(q.y);
        x1 = x1.max(q.x);
        y1 = y1.max(q.y);
    }
    p.x >= x0 - radius && p.x <= x1 + radius && p.y >= y0 - radius && p.y <= y1 + radius
}

/// Whether `p` lies inside the box around the points `first..=last` grown by `radius`.
/// Every segment of a contour lies inside the hull of its points, so a contour failing
/// this has no segment within `radius`, and its segments are never built. Points that
/// are NaN do not widen the box; the segments that use them are rejected anyway.
fn contour_is_near(coords: &[f64], first: usize, last: usize, p: Point, radius: f64) -> bool {
    let (mut x0, mut y0, mut x1, mut y1) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    let (pairs, _) = coords[2 * first..2 * (last + 1)].as_chunks::<2>();
    for [x, y] in pairs {
        x0 = x0.min(*x);
        y0 = y0.min(*y);
        x1 = x1.max(*x);
        y1 = y1.max(*y);
    }
    p.x >= x0 - radius && p.x <= x1 + radius && p.y >= y0 - radius && p.y <= y1 + radius
}

/// Every segment of every well-formed contour, in order, collected (used by the tests).
#[cfg(test)]
fn segments(coords: &[f64], flags: &[u8], contour_ends: &[u32]) -> Vec<Segment> {
    let mut out = Vec::new();
    for_each_segment(coords, flags, contour_ends, |_, _| true, |s| out.push(s));
    out
}

/// Calls `visit` for every segment of every well-formed contour, in order, without
/// allocating: a hit test runs once per pointer move and must stay well under a frame.
/// `keep(first, last)` is asked for each contour (its first and last point) and returns
/// false to skip it.
fn for_each_segment(
    coords: &[f64],
    flags: &[u8],
    contour_ends: &[u32],
    keep: impl Fn(usize, usize) -> bool,
    mut visit: impl FnMut(Segment),
) {
    let n = point_count(coords, flags);
    let mut start = 0usize;
    for (contour, &end) in contour_ends.iter().enumerate() {
        let end = end as usize;
        if end >= n || end < start {
            // Malformed contour: skip it; the next contour starts where this one would have.
            continue;
        }
        if keep(start, end) {
            contour_segments(coords, flags, contour, start, end, &mut visit);
        }
        start = end + 1;
    }
}

/// Segments of the closed contour made of points `first..=last`.
fn contour_segments(
    coords: &[f64],
    flags: &[u8],
    contour: usize,
    first: usize,
    last: usize,
    visit: &mut impl FnMut(Segment),
) {
    let len = last - first + 1;
    let is_on = |i: usize| flags[i] & flags::TYPE_MASK != flags::OFF_CURVE;
    // A contour of only off-curve points has no segments we can name by on-curve ends.
    let Some(anchor) = (first..=last).find(|&i| is_on(i)) else {
        return;
    };
    let mut prev_on = anchor;
    // The first two off-curve points since the last on-curve point, and how many there were.
    let mut offs = [0usize; 2];
    let mut off_count = 0usize;
    for step in 1..=len {
        let i = first + (anchor - first + step) % len;
        if !is_on(i) {
            if let Some(slot) = offs.get_mut(off_count) {
                *slot = i;
            }
            off_count += 1;
            continue;
        }
        let p0 = point_at(coords, prev_on);
        let p3 = point_at(coords, i);
        let seg = match (flags[i] & flags::TYPE_MASK, off_count) {
            (flags::CURVE, 2) => PathSeg::Cubic(CubicBez::new(
                p0,
                point_at(coords, offs[0]),
                point_at(coords, offs[1]),
                p3,
            )),
            (flags::QCURVE, 1) => PathSeg::Quad(QuadBez::new(p0, point_at(coords, offs[0]), p3)),
            // ponytail: other off-curve counts (TrueType implied points, super-curves) are
            // treated as a straight segment; split them into quads/cubics when a tool needs it.
            _ => PathSeg::Line(Line::new(p0, p3)),
        };
        visit(Segment {
            contour,
            start: prev_on,
            end: i,
            seg,
        });
        prev_on = i;
        off_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::ParamCurve;
    use proptest::prelude::*;

    const ON: u8 = flags::LINE;
    const OFF: u8 = flags::OFF_CURVE;
    const EPS: f64 = 1e-9;

    /// A 100×100 square with straight sides, points at the corners.
    fn square() -> (Vec<f64>, Vec<u8>, Vec<u32>) {
        (
            vec![0.0, 0.0, 100.0, 0.0, 100.0, 100.0, 0.0, 100.0],
            vec![ON; 4],
            vec![3],
        )
    }

    #[test]
    fn a_point_within_the_radius_is_hit() {
        let (c, f, e) = square();
        assert_eq!(
            hit_test(&c, &f, &e, 101.0, 99.0, 5.0),
            Some(Hit::Point {
                index: 2,
                distance: 2f64.sqrt()
            })
        );
    }

    #[test]
    fn the_nearest_of_several_points_wins_and_ties_keep_the_first() {
        let c = [0.0, 0.0, 4.0, 0.0, 2.0, 0.0];
        let f = [ON; 3];
        assert_eq!(
            hit_test(&c, &f, &[2], 2.5, 0.0, 10.0),
            Some(Hit::Point {
                index: 2,
                distance: 0.5
            })
        );
        assert_eq!(
            hit_test(&c, &f, &[2], 1.0, 0.0, 10.0),
            Some(Hit::Point {
                index: 0,
                distance: 1.0
            })
        );
    }

    #[test]
    fn a_line_segment_is_hit_when_no_point_is_near() {
        let (c, f, e) = square();
        assert_eq!(
            hit_test(&c, &f, &e, 50.0, 3.0, 5.0),
            Some(Hit::Segment {
                contour: 0,
                start: 0,
                end: 1,
                t: 0.5,
                distance: 3.0
            })
        );
    }

    #[test]
    fn the_closing_segment_of_a_contour_is_hit() {
        let (c, f, e) = square();
        let Some(Hit::Segment { start, end, .. }) = hit_test(&c, &f, &e, -2.0, 50.0, 5.0) else {
            panic!("expected a segment hit");
        };
        assert_eq!((start, end), (3, 0));
    }

    #[test]
    fn cubic_and_quadratic_segments_follow_the_curve() {
        // Contour 0: a cubic from (0,0) to (100,0) bulging up to y = 75 at t = 0.5.
        // Contour 1: a quadratic from (200,0) to (300,0) peaking at y = 50.
        let c = [
            0.0, 0.0, 0.0, 100.0, 100.0, 100.0, 100.0, 0.0, //
            200.0, 0.0, 250.0, 100.0, 300.0, 0.0,
        ];
        let f = [ON, OFF, OFF, flags::CURVE, ON, OFF, flags::QCURVE];
        let e = [3, 6];
        let Some(Hit::Segment {
            contour,
            start,
            end,
            t,
            distance,
        }) = hit_test(&c, &f, &e, 50.0, 77.0, 5.0)
        else {
            panic!("expected the cubic");
        };
        assert_eq!((contour, start, end), (0, 0, 3));
        assert!((t - 0.5).abs() < 1e-6 && (distance - 2.0).abs() < 1e-6);
        let Some(Hit::Segment {
            contour,
            start,
            end,
            distance,
            ..
        }) = hit_test(&c, &f, &e, 250.0, 51.0, 5.0)
        else {
            panic!("expected the quadratic");
        };
        assert_eq!((contour, start, end), (1, 4, 6));
        assert!((distance - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_point_beats_a_closer_segment() {
        let (c, f, e) = square();
        // 1 unit from the bottom edge, 4.1 from the corner: the corner wins.
        assert!(matches!(
            hit_test(&c, &f, &e, 4.0, 1.0, 5.0),
            Some(Hit::Point { index: 0, .. })
        ));
    }

    #[test]
    fn nothing_within_the_radius_is_none() {
        let (c, f, e) = square();
        assert_eq!(hit_test(&c, &f, &e, 50.0, 50.0, 5.0), None);
    }

    #[test]
    fn negative_or_nan_radius_and_nan_positions_hit_nothing() {
        let (c, f, e) = square();
        assert_eq!(hit_test(&c, &f, &e, 0.0, 0.0, -1.0), None);
        assert_eq!(hit_test(&c, &f, &e, 0.0, 0.0, f64::NAN), None);
        assert_eq!(hit_test(&c, &f, &e, f64::NAN, 0.0, 5.0), None);
    }

    #[test]
    fn an_infinite_radius_never_reports_a_point_at_infinite_distance() {
        let c = [f64::INFINITY, 0.0, 3.0, 4.0];
        let f = [ON; 2];
        assert_eq!(
            hit_test(&c, &f, &[1], 0.0, 0.0, f64::INFINITY),
            Some(Hit::Point {
                index: 1,
                distance: 5.0
            })
        );
        assert_eq!(
            hit_test(&c[..2], &f[..1], &[0], 0.0, 0.0, f64::INFINITY),
            None
        );
    }

    #[test]
    fn empty_and_malformed_outlines_do_not_panic() {
        assert_eq!(hit_test(&[], &[], &[], 0.0, 0.0, 5.0), None);
        // Odd coordinate count, fewer flags than points, contour ends out of order and range:
        // only the first two points exist, as two one-point contours.
        let c = [0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 7.0];
        let f = [ON, ON];
        let e = [9, 0, 0, 1];
        assert_eq!(segments(&c, &f, &e).len(), 2);
        assert_eq!(hit_test(&c, &f, &e, 5.0, 1.0, 2.0), None);
        assert_eq!(
            hit_test(&c, &f, &e, 10.0, 10.0, 20.0),
            Some(Hit::Point {
                index: 1,
                distance: 10.0
            })
        );
    }

    #[test]
    fn malformed_contours_are_skipped_but_their_points_still_hit() {
        // Contour end 9 is out of range; contour end 0 after it is still usable.
        let c = [0.0, 0.0, 10.0, 0.0, 10.0, 10.0];
        let f = [ON; 3];
        assert_eq!(segments(&c, &f, &[9, 2]).len(), 3);
        assert_eq!(segments(&c, &f, &[1, 0]).len(), 2);
        assert!(matches!(
            hit_test(&c, &f, &[9], 10.0, 9.0, 2.0),
            Some(Hit::Point { index: 2, .. })
        ));
        assert_eq!(hit_test(&c, &f, &[9], 5.0, 1.0, 2.0), None);
    }

    #[test]
    fn contours_of_only_off_curve_points_have_no_segments() {
        let c = [0.0, 0.0, 10.0, 0.0, 10.0, 10.0];
        assert!(segments(&c, &[OFF; 3], &[2]).is_empty());
    }

    #[test]
    fn unexpected_off_curve_counts_become_straight_segments() {
        // A `curve` point with one off-curve and a `line` point after two.
        let c = [
            0.0, 0.0, 5.0, 5.0, 10.0, 0.0, 12.0, 1.0, 13.0, 1.0, 20.0, 0.0,
        ];
        let f = [ON, OFF, flags::CURVE, OFF, OFF, ON];
        let found = segments(&c, &f, &[5]);
        assert_eq!(found.len(), 3);
        assert!(found.iter().all(|s| matches!(s.seg, PathSeg::Line(_))));
    }

    #[test]
    fn segments_with_absurd_coordinates_cannot_be_hit_but_their_points_can() {
        let f = [ON; 2];
        for far in [f64::INFINITY, f64::NAN, 2.0 * MAX_COORD, -2.0 * MAX_COORD] {
            let c = [0.0, 0.0, far, 0.0];
            assert_eq!(hit_test(&c, &f, &[1], 5.0, 1.0, 2.0), None);
            assert!(matches!(
                hit_test(&c, &f, &[1], 0.0, 1.0, 2.0),
                Some(Hit::Point { index: 0, .. })
            ));
        }
        // The limit itself is still hit-testable.
        let c = [0.0, 0.0, MAX_COORD, 0.0];
        assert!(matches!(
            hit_test(&c, &f, &[1], 5.0, 1.0, 2.0),
            Some(Hit::Segment { .. })
        ));
    }

    #[test]
    fn translate_moves_only_the_selection_and_leaves_the_input_untouched() {
        let c = vec![0.0, 0.0, 1.0, 1.0, 2.0, 2.0];
        let moved = translate_points(&c, &[2, 0], 10.0, -1.0);
        assert_eq!(moved, [10.0, -1.0, 1.0, 1.0, 12.0, 1.0]);
        assert_eq!(c, [0.0, 0.0, 1.0, 1.0, 2.0, 2.0]);
    }

    #[test]
    fn translate_ignores_duplicate_and_out_of_range_indices() {
        let moved = translate_points(&[0.0, 0.0, 5.0], &[0, 0, 1, 7], 1.0, 1.0);
        assert_eq!(moved, [1.0, 1.0, 5.0]);
    }

    /// A random outline: coordinates, flags and contour ends that partition the points.
    fn outline() -> impl Strategy<Value = (Vec<f64>, Vec<u8>, Vec<u32>)> {
        (1usize..40)
            .prop_flat_map(|n| {
                (
                    prop::collection::vec(-500.0f64..500.0, 2 * n),
                    prop::collection::vec(0u8..8, n),
                    prop::collection::vec(any::<bool>(), n),
                )
            })
            .prop_map(|(coords, flags, cuts)| {
                let last = cuts.len() - 1;
                let ends = cuts
                    .iter()
                    .enumerate()
                    .filter(|&(i, &cut)| cut || i == last)
                    .map(|(i, _)| i as u32)
                    .collect();
                (coords, flags, ends)
            })
    }

    fn sampled_distance(seg: &PathSeg, p: Point) -> f64 {
        (0..=400)
            .map(|k| (seg.eval(f64::from(k) / 400.0) - p).hypot())
            .fold(f64::INFINITY, f64::min)
    }

    proptest! {
        #[test]
        fn hit_test_returns_the_nearest_item_within_the_radius_or_none(
            (c, f, e) in outline(),
            x in -600.0f64..600.0,
            y in -600.0f64..600.0,
            radius in 0.0f64..300.0,
        ) {
            let p = Point::new(x, y);
            let point_distances: Vec<f64> =
                (0..f.len()).map(|i| (point_at(&c, i) - p).hypot()).collect();
            let nearest_point = point_distances.iter().copied().fold(f64::INFINITY, f64::min);
            let segs = segments(&c, &f, &e);
            match hit_test(&c, &f, &e, x, y, radius) {
                // EPS absorbs the last-bit difference between `hypot` and `sqrt(hypot2)`.
                Some(Hit::Point { index, distance }) => {
                    prop_assert!(distance <= radius + EPS);
                    prop_assert!((distance - point_distances[index]).abs() < EPS);
                    prop_assert!((distance - nearest_point).abs() < EPS);
                }
                Some(Hit::Segment { start, end, t, distance, .. }) => {
                    prop_assert!(nearest_point > radius - EPS, "a point within the radius must win");
                    prop_assert!(distance <= radius + EPS);
                    let seg = segs.iter().find(|s| s.start == start && s.end == end).map(|s| s.seg);
                    prop_assert!(seg.is_some());
                    if let Some(seg) = seg {
                        prop_assert!(((seg.eval(t) - p).hypot() - distance).abs() < 1e-6);
                    }
                    for s in &segs {
                        prop_assert!(sampled_distance(&s.seg, p) >= distance - 1e-6);
                    }
                }
                None => {
                    prop_assert!(nearest_point > radius - EPS);
                    for s in &segs {
                        prop_assert!(sampled_distance(&s.seg, p) > radius - 1e-6);
                    }
                }
            }
        }

        #[test]
        fn hit_test_never_panics_on_arbitrary_input(
            c in prop::collection::vec(any::<f64>(), 0..40),
            f in prop::collection::vec(any::<u8>(), 0..20),
            e in prop::collection::vec(0u32..25, 0..8),
            x in any::<f64>(),
            y in any::<f64>(),
            radius in any::<f64>(),
        ) {
            let _ = hit_test(&c, &f, &e, x, y, radius);
        }

        #[test]
        fn translate_leaves_points_outside_the_selection_unchanged(
            c in prop::collection::vec(-1e6f64..1e6, 0..80),
            selection in prop::collection::vec(0u32..50, 0..30),
            dx in -1e3f64..1e3,
            dy in -1e3f64..1e3,
        ) {
            let moved = translate_points(&c, &selection, dx, dy);
            prop_assert_eq!(moved.len(), c.len());
            for (i, (&before, &after)) in c.iter().zip(&moved).enumerate() {
                let point = (i / 2) as u32;
                let selected = selection.contains(&point) && 2 * (i / 2) + 1 < c.len();
                let expected = match (selected, i % 2) {
                    (false, _) => before,
                    (true, 0) => before + dx,
                    (true, _) => before + dy,
                };
                prop_assert_eq!(after, expected);
            }
        }
    }
}
