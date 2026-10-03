//! Headless timing of `hit_test` and `translate_points` on 5,000-point glyphs, for the
//! M0 Step 8 budget (hit-test under 1 ms, implementation plan §7.3).
//!
//! Three glyph layouts, mirrored by `makeGlyph` and `makeNestedGlyph` in
//! `apps/desktop/src/bench/outline.ts`:
//! - grid: 50 contours of 100 points whose bounding boxes are apart. The best case: the hit
//!   test skips about 49 of 50 contours by their box;
//! - nested 2 x 2,500 and nested 5 x 1,000: few large concentric contours with the pointer
//!   inside every box. The worst case: no contour can be skipped.
//!
//! Run with `cargo bench -p tf-geometry`; it prints one line per case and needs no extra
//! dependencies.

use std::f64::consts::TAU;
use std::hint::black_box;
use std::time::Instant;

use tf_geometry::{flags, hit_test, translate_points};

const CALLS: usize = 2_000;
const HIT_RADIUS: f64 = 8.0;

struct Glyph {
    coords: Vec<f64>,
    flags: Vec<u8>,
    contour_ends: Vec<u32>,
}

/// A box in font units.
struct Area {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

/// `contours` wavy rings of `points_per_contour` points: one line point, then cubic segments
/// (off, off, curve). `place(c)` gives the centre and base radius of contour `c`.
fn build_rings(
    contours: usize,
    points_per_contour: usize,
    place: impl Fn(usize) -> (f64, f64, f64),
) -> Glyph {
    let total = contours * points_per_contour;
    let mut coords = Vec::with_capacity(2 * total);
    let mut point_flags = Vec::with_capacity(total);
    let mut contour_ends = Vec::with_capacity(contours);
    for c in 0..contours {
        let (cx, cy, base) = place(c);
        for i in 0..points_per_contour {
            let angle = TAU * i as f64 / points_per_contour as f64;
            let is_on = i % 3 == 0;
            let radius = base + if is_on { -2.5 } else { 2.5 } + 3.0 * (7.0 * angle).sin();
            coords.push(cx + radius * angle.cos());
            coords.push(cy + radius * angle.sin());
            point_flags.push(match (i, is_on) {
                (0, _) => flags::LINE,
                (_, true) => flags::CURVE,
                _ => flags::OFF_CURVE,
            });
        }
        contour_ends.push(((c + 1) * points_per_contour - 1) as u32);
    }
    Glyph {
        coords,
        flags: point_flags,
        contour_ends,
    }
}

/// 50 contours of 100 points on a grid, bounding boxes apart.
fn make_grid_glyph() -> Glyph {
    let columns = 8; // ceil(sqrt(50))
    build_rings(50, 100, |c| {
        (
            60.0 + (c % columns) as f64 * 110.0,
            60.0 + (c / columns) as f64 * 110.0,
            47.5,
        )
    })
}

/// `contours` concentric rings; the outermost has about 3 units between points.
fn make_nested_glyph(contours: usize, points_per_contour: usize) -> Glyph {
    let outer = points_per_contour as f64 / 2.0;
    build_rings(contours, points_per_contour, |c| {
        (
            outer + 60.0,
            outer + 60.0,
            outer * (contours - c) as f64 / contours as f64,
        )
    })
}

/// The intersection of the bounding boxes of all contours: for a nested glyph, the box of
/// the innermost ring.
fn inner_box(glyph: &Glyph) -> Area {
    let mut result = Area {
        x0: f64::NEG_INFINITY,
        y0: f64::NEG_INFINITY,
        x1: f64::INFINITY,
        y1: f64::INFINITY,
    };
    let mut first = 0usize;
    for &last in &glyph.contour_ends {
        let last = last as usize;
        let (mut x0, mut y0, mut x1, mut y1) = (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        );
        let (pairs, _) = glyph.coords[2 * first..2 * (last + 1)].as_chunks::<2>();
        for [x, y] in pairs {
            x0 = x0.min(*x);
            x1 = x1.max(*x);
            y0 = y0.min(*y);
            y1 = y1.max(*y);
        }
        result.x0 = result.x0.max(x0);
        result.y0 = result.y0.max(y0);
        result.x1 = result.x1.min(x1);
        result.y1 = result.y1.min(y1);
        first = last + 1;
    }
    result
}

/// Deterministic positions inside `area` (Park-Miller, like the page).
fn positions(count: usize, area: &Area) -> Vec<(f64, f64)> {
    let mut seed = 1u64;
    let mut next = move || {
        seed = seed * 16_807 % 2_147_483_647;
        seed as f64 / 2_147_483_647.0
    };
    (0..count)
        .map(|_| {
            (
                area.x0 + next() * (area.x1 - area.x0),
                area.y0 + next() * (area.y1 - area.y0),
            )
        })
        .collect()
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let rank = (p / 100.0 * sorted.len() as f64).ceil() as usize;
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

fn report(name: &str, mut samples_ms: Vec<f64>) {
    samples_ms.sort_by(f64::total_cmp);
    let mean = samples_ms.iter().sum::<f64>() / samples_ms.len() as f64;
    println!(
        "{name}: n={} mean={mean:.4} ms p50={:.4} ms p95={:.4} ms p99={:.4} ms max={:.4} ms",
        samples_ms.len(),
        percentile(&samples_ms, 50.0),
        percentile(&samples_ms, 95.0),
        percentile(&samples_ms, 99.0),
        samples_ms[samples_ms.len() - 1],
    );
}

/// Times `hit_test` at radius 8 and radius 1 over `spots` on `glyph`.
fn bench_hit_test(label: &str, glyph: &Glyph, spots: &[(f64, f64)]) {
    // Radius 8 mostly finds points (they are about 3 units apart); radius 1 finds segments.
    for radius in [HIT_RADIUS, 1.0] {
        let (mut points, mut segments, mut nothing) = (0, 0, 0);
        let mut samples = Vec::with_capacity(spots.len());
        for &(x, y) in spots {
            let start = Instant::now();
            let hit = hit_test(
                black_box(&glyph.coords),
                black_box(&glyph.flags),
                black_box(&glyph.contour_ends),
                x,
                y,
                radius,
            );
            samples.push(start.elapsed().as_secs_f64() * 1000.0);
            match hit {
                Some(tf_geometry::Hit::Point { .. }) => points += 1,
                Some(tf_geometry::Hit::Segment { .. }) => segments += 1,
                None => nothing += 1,
            }
        }
        println!(
            "{label}, radius {radius}: {points} points, {segments} segments, {nothing} misses"
        );
        report(&format!("hit_test, {label}, radius {radius}"), samples);
    }
}

fn main() {
    let glyph = make_grid_glyph();
    let everywhere = Area {
        x0: 0.0,
        y0: 0.0,
        x1: 900.0,
        y1: 800.0,
    };
    bench_hit_test(
        "grid 50 x 100 (best case)",
        &glyph,
        &positions(CALLS, &everywhere),
    );

    // The worst case: the pointer is inside the box of every contour.
    for (contours, points) in [(2, 2500), (5, 1000)] {
        let nested = make_nested_glyph(contours, points);
        let spots = positions(CALLS, &inner_box(&nested));
        bench_hit_test(
            &format!("nested {contours} x {points} (worst case)"),
            &nested,
            &spots,
        );
    }

    // The same position every call, in the first row of contours: shows the cost without
    // the variation of the random positions.
    let crowded: Vec<f64> = (0..CALLS)
        .map(|_| {
            let start = Instant::now();
            let _ = black_box(hit_test(
                black_box(&glyph.coords),
                black_box(&glyph.flags),
                black_box(&glyph.contour_ends),
                105.0,
                105.0,
                HIT_RADIUS,
            ));
            start.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    report("hit_test (grid, one crowded spot)", crowded);

    let selection: Vec<u32> = (0..1000).collect();
    let moved: Vec<f64> = (0..CALLS)
        .map(|i| {
            let start = Instant::now();
            let _ = black_box(translate_points(
                black_box(&glyph.coords),
                black_box(&selection),
                i as f64,
                1.0,
            ));
            start.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    report("translate_points (5,000 points, 1,000 selected)", moved);
}
