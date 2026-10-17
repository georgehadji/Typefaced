//! Property tests of union, intersection and difference (implementation plan §5.2) on
//! generated pairs of shapes, measured with the scanline oracle.
//!
//! Each output is compared with an exact reference built from the inputs alone. Both
//! inputs are first wound so that every filled point has a positive winding number; then
//! the two paths joined fill exactly a ∪ b under the non-zero rule. So:
//! - union: the output fills the same region as a and b joined;
//! - difference: the output and b joined fill a ∪ b, and their areas add up to it;
//! - intersection: the output lies inside a and inside b, with area a + b − (a ∪ b).
//!
//! The area invariants of the plan follow from these. Every output must also have one
//! winding direction (outer contours counter-clockwise) and no overlaps.

// Each test target uses only part of the shared module.
#[allow(dead_code)]
mod common;

use common::cases::{circle, join, polygon, rect};
use common::oracle;
use kurbo::BezPath;
use proptest::prelude::*;
use tf_boolean::Linesweeper;
use tf_geometry::BooleanEngine;

/// A rectangle with integer corners, as in a drawn font outline.
fn int_rect() -> impl Strategy<Value = (i32, i32, i32, i32)> {
    (0..600, 0..600, 10..400, 10..400)
}

/// One primitive, counter-clockwise: a rectangle or a ring (a rectangle with a counter) on
/// integer coordinates, so edges and corners of different primitives often coincide; a
/// circle with integer centre and radius; or a star-shaped polygon (simple).
fn primitive() -> impl Strategy<Value = BezPath> {
    prop_oneof![
        int_rect().prop_map(|(x, y, w, h)| rect(
            f64::from(x),
            f64::from(y),
            f64::from(x + w),
            f64::from(y + h)
        )),
        (int_rect(), 1..20i32, 1..20i32).prop_map(|((x, y, w, h), mx, my)| {
            let (w, h) = (w + 2 * mx + 10, h + 2 * my + 10);
            let outer = rect(
                f64::from(x),
                f64::from(y),
                f64::from(x + w),
                f64::from(y + h),
            );
            let counter = rect(
                f64::from(x + mx),
                f64::from(y + my),
                f64::from(x + w - mx),
                f64::from(y + h - my),
            );
            join([outer, counter.reverse_subpaths()])
        }),
        (0..600, 0..600, 6..250).prop_map(|(x, y, r)| circle(
            f64::from(x),
            f64::from(y),
            f64::from(r),
            true
        )),
        (
            0.0..600.0,
            0.0..600.0,
            prop::collection::vec((0.0..1.0f64, 20.0..300.0f64), 3..12)
        )
            .prop_map(|(cx, cy, mut spokes)| {
                spokes.sort_by(|a, b| a.0.total_cmp(&b.0));
                let points: Vec<(f64, f64)> = spokes
                    .iter()
                    .enumerate()
                    .map(|(i, &(jitter, r))| {
                        // Angles strictly increase, so the polygon is simple.
                        let t =
                            std::f64::consts::TAU * (i as f64 + 0.9 * jitter) / spokes.len() as f64;
                        (cx + r * t.cos(), cy + r * t.sin())
                    })
                    .collect();
                polygon(&points)
            }),
    ]
}

/// A shape: one to three primitives that may overlap one another, counter-clockwise (the
/// reference), and the same shape as the engine gets it: reversed half of the time, which
/// fills the same region under the non-zero rule.
fn shape() -> impl Strategy<Value = (BezPath, BezPath)> {
    (prop::collection::vec(primitive(), 1..4), any::<bool>()).prop_map(|(parts, reversed)| {
        let reference = join(parts);
        let input = if reversed {
            reference.reverse_subpaths()
        } else {
            reference.clone()
        };
        (reference, input)
    })
}

/// Area of `path` under the non-zero rule, on the scanlines of `frame`.
///
/// The oracle's error is mostly the vertical step times the length of horizontal edges,
/// and it depends on where the scanlines fall. Every area of one test case is measured on
/// the scanlines of a ∪ b (the outputs lie inside it), so these errors cancel when areas
/// are added and compared.
fn area(frame: &BezPath, path: &BezPath) -> f64 {
    oracle::compare(frame, path).output_area
}

/// The area filled by exactly one of `reference` and `output`.
fn xor(reference: &BezPath, output: &BezPath) -> f64 {
    oracle::compare(reference, output).xor_area
}

proptest! {
    // Each case runs three operations and about fifteen oracle scans in a debug build.
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn outputs_match_the_exact_set_operations((a, a_in) in shape(), (b, b_in) in shape()) {
        let a_or_b = join([a.clone(), b.clone()]);
        let in_frame = |path: &BezPath| area(&a_or_b, path);
        let (area_a, area_b, area_ab) = (in_frame(&a), in_frame(&b), in_frame(&a_or_b));
        // What remains of the oracle's error (flattening, and outputs that match the input
        // only to within the engine's accuracy). The smallest primitive is 100 unit², so a
        // dropped or added primitive is always above it.
        let tolerance = 1e-4 * area_ab + 2.0;
        let fail = |e: tf_geometry::BooleanError| TestCaseError::fail(e.to_string());
        let union = Linesweeper.union(&a_in, &b_in).map_err(fail)?;
        let inter = Linesweeper.intersection(&a_in, &b_in).map_err(fail)?;
        let diff = Linesweeper.difference(&a_in, &b_in).map_err(fail)?;

        let e = xor(&a_or_b, &union);
        prop_assert!(e <= tolerance, "union differs from a ∪ b by {e}");

        let e = xor(&a_or_b, &join([diff.clone(), b.clone()]));
        prop_assert!(e <= tolerance, "(a − b) ∪ b differs from a ∪ b by {e}");
        let (area_diff, area_inter) = (in_frame(&diff), in_frame(&inter));
        prop_assert!((area_diff + area_b - area_ab).abs() <= tolerance,
            "area(a − b) {area_diff} + area(b) {area_b} != area(a ∪ b) {area_ab}");

        for (name, outer) in [("a", &a), ("b", &b)] {
            let e = xor(outer, &join([outer.clone(), inter.clone()]));
            prop_assert!(e <= tolerance, "a ∩ b sticks out of {name} by {e}");
        }
        prop_assert!((area_inter - (area_a + area_b - area_ab)).abs() <= tolerance,
            "area(a ∩ b) {area_inter} != {area_a} + {area_b} - {area_ab}");

        // One winding direction (outer contours counter-clockwise) and no overlaps.
        for (name, out) in [("union", &union), ("intersection", &inter), ("difference", &diff)] {
            let profile = oracle::compare(&BezPath::new(), out);
            prop_assert!(profile.overlap_area <= tolerance && profile.negative_area <= tolerance,
                "{name}: {profile:?}");
        }
    }
}
