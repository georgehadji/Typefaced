//! Property tests of the area invariants (implementation plan §5.2): for generated pairs of
//! shapes, area(a ∪ b) ≥ max(area(a), area(b)), area(a ∩ b) ≤ min(area(a), area(b)) and
//! area(a − b) = area(a) − area(a ∩ b), all measured with the scanline oracle.

#[allow(dead_code)]
mod common;

use common::cases::{circle, join, polygon, rect};
use common::oracle;
use kurbo::BezPath;
use proptest::prelude::*;
use tf_boolean::Linesweeper;
use tf_geometry::BooleanEngine;

/// One primitive: a rectangle, a circle, or a star-shaped polygon (simple, counter-clockwise).
fn primitive() -> impl Strategy<Value = BezPath> {
    prop_oneof![
        (0.0..600.0, 0.0..600.0, 5.0..400.0, 5.0..400.0).prop_map(|(x, y, w, h)| rect(
            x,
            y,
            x + w,
            y + h
        )),
        (0.0..600.0, 0.0..600.0, 5.0..250.0).prop_map(|(x, y, r)| circle(x, y, r, true)),
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

/// A shape: one to three primitives that may overlap one another.
fn shape() -> impl Strategy<Value = BezPath> {
    prop::collection::vec(primitive(), 1..4).prop_map(join)
}

/// Area under the non-zero rule.
fn area(path: &BezPath) -> f64 {
    oracle::compare(path, &BezPath::new()).reference_area
}

proptest! {
    // Each case runs three operations and four oracle scans in a debug build.
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn areas_obey_set_inequalities(a in shape(), b in shape()) {
        let (area_a, area_b) = (area(&a), area(&b));
        // The oracle's error: flattening and the vertical step, relative to the shape.
        let tolerance = 1e-3 * area_a.max(area_b) + 1.0;
        let union = Linesweeper.union(&a, &b).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let inter = Linesweeper.intersection(&a, &b).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let diff = Linesweeper.difference(&a, &b).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let (area_union, area_inter, area_diff) = (area(&union), area(&inter), area(&diff));
        prop_assert!(area_union >= area_a.max(area_b) - tolerance, "{area_union} < max({area_a}, {area_b})");
        prop_assert!(area_inter <= area_a.min(area_b) + tolerance, "{area_inter} > min({area_a}, {area_b})");
        prop_assert!((area_diff - (area_a - area_inter)).abs() <= tolerance,
            "{area_diff} != {area_a} - {area_inter}");
        // Inclusion-exclusion ties the three together.
        prop_assert!((area_union - (area_a + area_b - area_inter)).abs() <= tolerance,
            "{area_union} != {area_a} + {area_b} - {area_inter}");
        // The outputs are clean: no overlaps and one winding direction for filled regions.
        for out in [&union, &inter, &diff] {
            let profile = oracle::compare(&BezPath::new(), out);
            prop_assert!(profile.overlap_area <= tolerance && profile.mixed_winding_area() <= tolerance,
                "{profile:?}");
        }
    }
}
