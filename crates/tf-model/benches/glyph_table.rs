//! Regression benchmark for the single-glyph edit budget (ADR-0017, M0 carry-over): replace
//! one glyph of 100 points at the first, middle and last position of a 1,000, 30,000 and
//! 65,535-glyph font, 1,000 samples each.
//!
//! Each sample clones the font (an undo snapshot keeps the old one alive, so `imbl` must copy
//! the path) and calls `Font::replace_glyph`, invariant checks included. Exits with status 1
//! when the middle replace at 30,000 glyphs has p95 >= 50 µs. Run with
//! `cargo bench -p tf-model --bench glyph_table`.

use std::hint::black_box;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use tf_model::{
    Contour, Font, Glyph, GlyphId, GlyphLayer, LayerId, ModelError, Name, Point, PointKind, Source,
    SourceId, SourceLayer,
};

const SIZES: [u32; 3] = [1_000, 30_000, 65_535];
const SAMPLES: usize = 1_000;
const BUDGET_US: f64 = 50.0;
const GATED_SIZE: u32 = 30_000;
const LAYER: LayerId = LayerId(0);

/// 100 points: four closed contours of one line point and eight cubic segments.
fn outline(dx: f64) -> GlyphLayer {
    let contours = (0..4)
        .map(|c| {
            let points = (0..25)
                .map(|i| {
                    let kind = match i {
                        0 => PointKind::Line,
                        _ if i % 3 == 0 => PointKind::Curve,
                        _ => PointKind::OffCurve,
                    };
                    Point::new(dx + f64::from(i) * 10.0, f64::from(c * 100 + i), kind)
                })
                .collect();
            Contour::new(points)
        })
        .collect();
    GlyphLayer {
        width: 600.0,
        contours,
        ..GlyphLayer::default()
    }
}

fn glyph(name: &str, layer: Arc<GlyphLayer>) -> Result<Arc<Glyph>, ModelError> {
    let mut glyph = Glyph::new(Name::new(name)?);
    glyph.layers.insert(LAYER, layer);
    Ok(Arc::new(glyph))
}

/// A font of `size` glyphs. They share one outline: the table cost does not depend on it,
/// and 65,535 distinct 100-point glyphs would take about 400 MB.
fn font(size: u32) -> Result<Font, ModelError> {
    let source = Source::new(SourceLayer::new(LAYER, Name::new("public.default")?));
    let mut font = Font::new(SourceId(0), source)?;
    let shared = Arc::new(outline(0.0));
    for i in 0..size {
        font.insert_glyph(glyph(&format!("g{i}"), Arc::clone(&shared))?)?;
    }
    Ok(font)
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let rank = (p / 100.0 * sorted.len() as f64).ceil() as usize;
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

/// Prints one line and returns p95 in µs.
fn report(name: &str, mut samples_us: Vec<f64>) -> f64 {
    samples_us.sort_by(f64::total_cmp);
    let p95 = percentile(&samples_us, 95.0);
    println!(
        "{name}: n={} p50={:.2} µs p95={p95:.2} µs p99={:.2} µs max={:.2} µs",
        samples_us.len(),
        percentile(&samples_us, 50.0),
        percentile(&samples_us, 99.0),
        samples_us[samples_us.len() - 1],
    );
    p95
}

fn replace_samples(base: &Font, id: GlyphId) -> Result<Vec<f64>, ModelError> {
    let name = format!("g{}", id.0);
    let edited = glyph(&name, Arc::new(outline(1.0)))?;
    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let start = Instant::now();
        let mut next = black_box(base).clone();
        let old = next.replace_glyph(id, Arc::clone(&edited))?;
        samples.push(start.elapsed().as_secs_f64() * 1e6);
        // Freeing the copied path is not part of the edit.
        drop(black_box((next, old)));
    }
    Ok(samples)
}

fn run() -> Result<bool, ModelError> {
    let mut within_budget = true;
    for size in SIZES {
        let base = font(size)?;
        for (position, id) in [("first", 0), ("middle", size / 2), ("last", size - 1)] {
            let p95 = report(
                &format!("replace_glyph, {size} glyphs, {position}"),
                replace_samples(&base, GlyphId(id))?,
            );
            if size == GATED_SIZE && position == "middle" && p95 >= BUDGET_US {
                eprintln!(
                    "FAIL: middle replace at {size} glyphs: p95 {p95:.2} µs >= {BUDGET_US} µs"
                );
                within_budget = false;
            }
        }
    }
    Ok(within_budget)
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("benchmark setup failed: {error}");
            ExitCode::FAILURE
        }
    }
}
