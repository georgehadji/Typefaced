//! Headless timing of `hit_test` and `translate_points` on the 5,000-point glyph of the
//! M0 Step 8 budgets (hit-test under 1 ms, implementation plan §7.3).
//!
//! The glyph is the same synthetic one as `makeGlyph()` in `apps/desktop/src/bench/outline.ts`:
//! 50 contours of 100 points, each one line point followed by cubic segments. Run with
//! `cargo bench -p tf-geometry`; it prints one line per case and needs no extra dependencies.

use std::f64::consts::TAU;
use std::hint::black_box;
use std::time::Instant;

use tf_geometry::{flags, hit_test, translate_points};

const CONTOURS: usize = 50;
const POINTS_PER_CONTOUR: usize = 100;
const CALLS: usize = 2_000;
const HIT_RADIUS: f64 = 8.0;

struct Glyph {
    coords: Vec<f64>,
    flags: Vec<u8>,
    contour_ends: Vec<u32>,
}

fn make_glyph() -> Glyph {
    let total = CONTOURS * POINTS_PER_CONTOUR;
    let columns = (CONTOURS as f64).sqrt().ceil() as usize;
    let mut coords = Vec::with_capacity(2 * total);
    let mut point_flags = Vec::with_capacity(total);
    let mut contour_ends = Vec::with_capacity(CONTOURS);
    for c in 0..CONTOURS {
        let cx = 60.0 + (c % columns) as f64 * 110.0;
        let cy = 60.0 + (c / columns) as f64 * 110.0;
        for i in 0..POINTS_PER_CONTOUR {
            let angle = TAU * i as f64 / POINTS_PER_CONTOUR as f64;
            let is_on = i % 3 == 0;
            let radius = (if is_on { 45.0 } else { 50.0 }) + 3.0 * (7.0 * angle).sin();
            coords.push(cx + radius * angle.cos());
            coords.push(cy + radius * angle.sin());
            point_flags.push(match (i, is_on) {
                (0, _) => flags::LINE,
                (_, true) => flags::CURVE,
                _ => flags::OFF_CURVE,
            });
        }
        contour_ends.push(((c + 1) * POINTS_PER_CONTOUR - 1) as u32);
    }
    Glyph {
        coords,
        flags: point_flags,
        contour_ends,
    }
}

/// Deterministic positions over the glyph's bounding box (Park-Miller, like the page).
fn positions(count: usize) -> Vec<(f64, f64)> {
    let mut seed = 1u64;
    let mut next = move || {
        seed = seed * 16_807 % 2_147_483_647;
        seed as f64 / 2_147_483_647.0
    };
    (0..count)
        .map(|_| (next() * 900.0, next() * 800.0))
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

fn main() {
    let glyph = make_glyph();
    let spots = positions(CALLS);
    // Radius 8 mostly finds points (they are about 3 units apart); radius 1 finds segments.
    for radius in [HIT_RADIUS, 1.0] {
        let (mut points, mut segments, mut nothing) = (0, 0, 0);
        let mut samples = Vec::with_capacity(CALLS);
        for &(x, y) in &spots {
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
        println!("radius {radius}: {points} points, {segments} segments, {nothing} misses");
        report(
            &format!("hit_test, radius {radius} (5,000 points, 50 contours)"),
            samples,
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
    report("hit_test (one crowded spot)", crowded);

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
