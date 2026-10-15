//! Headless timing of the linesweeper engine on the fixture and synthetic cases: remove
//! overlap on each case, and intersection and difference of each case with a copy of
//! itself shifted by a third of its size (the Spike 5 follow-up). Prints p50, p95 and max
//! per operation and group; no budget. Run with `cargo bench -p tf-boolean`.

#[allow(dead_code)]
#[path = "../tests/common/mod.rs"]
mod common;

use std::hint::black_box;
use std::process::ExitCode;
use std::time::Instant;

use common::cases::{self, Case};
use kurbo::{Affine, BezPath, Shape};
use tf_boolean::Linesweeper;
use tf_geometry::{BooleanEngine, BooleanError};

/// Runs per case; the median of these is the case's time.
const REPS: usize = 7;

type Op = fn(&BezPath, &BezPath) -> Result<BezPath, BooleanError>;

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let rank = (p / 100.0 * sorted.len() as f64).ceil() as usize;
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

/// The median time of `op(a, b)` in microseconds.
fn time(op: Op, a: &BezPath, b: &BezPath) -> Result<f64, BooleanError> {
    let mut samples = Vec::with_capacity(REPS);
    for _ in 0..REPS {
        let start = Instant::now();
        black_box(op(black_box(a), black_box(b))?);
        samples.push(start.elapsed().as_secs_f64() * 1e6);
    }
    samples.sort_by(f64::total_cmp);
    Ok(samples[REPS / 2])
}

fn shifted(path: &BezPath) -> BezPath {
    let bbox = path.bounding_box();
    Affine::translate((bbox.width() / 3.0, bbox.height() / 3.0)) * path.clone()
}

fn bench(group: &str, cases: &[Case]) -> Result<(), String> {
    let ops: [(&str, Op, bool); 3] = [
        (
            "remove overlap",
            |a, _| Linesweeper.remove_overlap(a),
            false,
        ),
        ("intersection", |a, b| Linesweeper.intersection(a, b), true),
        ("difference", |a, b| Linesweeper.difference(a, b), true),
    ];
    for (name, op, binary) in ops {
        let mut micros = Vec::with_capacity(cases.len());
        for case in cases {
            let other = if binary {
                shifted(&case.path)
            } else {
                BezPath::new()
            };
            micros.push(time(op, &case.path, &other).map_err(|e| format!("{}: {e}", case.name))?);
        }
        micros.sort_by(f64::total_cmp);
        println!(
            "{name}, {group} ({} cases): p50={:.0} us p95={:.0} us max={:.0} us",
            micros.len(),
            percentile(&micros, 50.0),
            percentile(&micros, 95.0),
            micros[micros.len() - 1],
        );
    }
    Ok(())
}

fn main() -> ExitCode {
    let run = || -> Result<(), String> {
        bench("fixtures", &cases::fixtures()?)?;
        bench("synthetic", &cases::synthetic())
    };
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
