//! The two candidate engines behind one function: non-zero remove-overlap.

use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use kurbo::BezPath;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Engine {
    /// `Simplify`, then `AsWinding`: the candidate.
    #[cfg(feature = "skia")]
    Skia,
    /// `Simplify` alone. Kept as evidence: its output is meant for the even-odd
    /// rule, so contour directions are not consistent under non-zero.
    #[cfg(feature = "skia")]
    SkiaSimplifyOnly,
    /// PathOps `Op` union with an empty path, no direction fix. Kept as evidence
    /// that the direction failures do not come from choosing `Simplify`.
    #[cfg(feature = "skia")]
    SkiaUnionOnly,
    #[cfg(feature = "sweep")]
    Linesweeper,
}

/// Engines compiled into this build (see the crate features).
pub const ENGINES: &[Engine] = &[
    #[cfg(feature = "skia")]
    Engine::Skia,
    #[cfg(feature = "skia")]
    Engine::SkiaSimplifyOnly,
    #[cfg(feature = "skia")]
    Engine::SkiaUnionOnly,
    #[cfg(feature = "sweep")]
    Engine::Linesweeper,
];

/// The two candidates (benchmarked); `SkiaSimplifyOnly` is a diagnostic.
pub const CANDIDATES: &[Engine] = &[
    #[cfg(feature = "skia")]
    Engine::Skia,
    #[cfg(feature = "sweep")]
    Engine::Linesweeper,
];

impl Engine {
    pub fn name(self) -> &'static str {
        match self {
            #[cfg(feature = "skia")]
            Engine::Skia => "skia-pathops",
            #[cfg(feature = "skia")]
            Engine::SkiaSimplifyOnly => "skia-simplify-only",
            #[cfg(feature = "skia")]
            Engine::SkiaUnionOnly => "skia-union-only",
            #[cfg(feature = "sweep")]
            Engine::Linesweeper => "linesweeper",
        }
    }

    /// Removes overlaps under the non-zero fill rule.
    pub fn remove_overlaps(self, path: &BezPath) -> Result<BezPath, String> {
        match self {
            #[cfg(feature = "skia")]
            Engine::Skia => skia::remove_overlaps(path, Variant::SimplifyAsWinding),
            #[cfg(feature = "skia")]
            Engine::SkiaSimplifyOnly => skia::remove_overlaps(path, Variant::Simplify),
            #[cfg(feature = "skia")]
            Engine::SkiaUnionOnly => skia::remove_overlaps(path, Variant::UnionWithEmpty),
            #[cfg(feature = "sweep")]
            Engine::Linesweeper => sweep::union(path),
        }
    }
}

/// What happened when an engine ran on one case.
pub enum Outcome {
    Ok(BezPath, Duration),
    Error(String),
    Panic(String),
    Timeout,
}

/// Runs one engine on its own thread, so a panic or a hang is recorded instead
/// of ending the run. A hung thread is leaked. A crash in native code (Skia)
/// still ends the process.
pub fn run_guarded(engine: Engine, path: &BezPath, timeout: Duration) -> Outcome {
    let (tx, rx) = mpsc::channel();
    let input = path.clone();
    let handle = thread::spawn(move || {
        let start = Instant::now();
        let result = engine.remove_overlaps(&input);
        // The receiver is gone only after a timeout; nothing is waiting then.
        let _ = tx.send((result, start.elapsed()));
    });
    match rx.recv_timeout(timeout) {
        Ok((Ok(out), elapsed)) => Outcome::Ok(out, elapsed),
        Ok((Err(e), _)) => Outcome::Error(e),
        Err(RecvTimeoutError::Timeout) => Outcome::Timeout,
        Err(RecvTimeoutError::Disconnected) => Outcome::Panic(match handle.join() {
            Err(payload) => payload
                .downcast_ref::<&str>()
                .map(|s| (*s).to_owned())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "non-string panic payload".to_owned()),
            Ok(()) => "thread ended without a result".to_owned(),
        }),
    }
}

#[cfg(feature = "skia")]
use skia::Variant;

#[cfg(feature = "skia")]
mod skia {
    use kurbo::{BezPath, PathEl};
    use skia_safe::{Path, PathBuilder, PathFillType, PathVerb, Point};

    /// Skia stores coordinates as f32; the conversion is part of the engine's cost.
    fn pt(p: kurbo::Point) -> Point {
        Point::new(p.x as f32, p.y as f32)
    }

    fn kp(p: Point) -> kurbo::Point {
        kurbo::Point::new(f64::from(p.x), f64::from(p.y))
    }

    /// Which PathOps calls make up the engine.
    #[derive(Clone, Copy)]
    pub enum Variant {
        SimplifyAsWinding,
        Simplify,
        UnionWithEmpty,
    }

    pub fn remove_overlaps(path: &BezPath, variant: Variant) -> Result<BezPath, String> {
        let mut b = PathBuilder::new_with_fill_type(PathFillType::Winding);
        for el in path.iter() {
            match el {
                PathEl::MoveTo(p) => b.move_to(pt(p)),
                PathEl::LineTo(p) => b.line_to(pt(p)),
                PathEl::QuadTo(p1, p2) => b.quad_to(pt(p1), pt(p2)),
                PathEl::CurveTo(p1, p2, p3) => b.cubic_to(pt(p1), pt(p2), pt(p3)),
                PathEl::ClosePath => b.close(),
            };
        }
        let input = b.detach();
        let out = match variant {
            Variant::UnionWithEmpty => {
                skia_safe::op(&input, &Path::new(), skia_safe::PathOp::Union)
                    .ok_or_else(|| "PathOps Op failed".to_owned())?
            }
            Variant::Simplify | Variant::SimplifyAsWinding => {
                let simplified = skia_safe::simplify(&input)
                    .ok_or_else(|| "PathOps Simplify failed".to_owned())?;
                if matches!(variant, Variant::SimplifyAsWinding) {
                    // Simplify returns non-overlapping contours for the even-odd rule;
                    // AsWinding re-orients them for the non-zero rule fonts use.
                    skia_safe::as_winding(&simplified)
                        .ok_or_else(|| "PathOps AsWinding failed".to_owned())?
                } else {
                    simplified
                }
            }
        };
        to_kurbo(&out)
    }

    fn to_kurbo(path: &Path) -> Result<BezPath, String> {
        let points = path.points();
        let mut i = 0;
        let mut take = |n: usize| {
            let s = points.get(i..i + n).ok_or("verb and point arrays disagree");
            i += n;
            s
        };
        let mut out = BezPath::new();
        for verb in path.verbs() {
            match verb {
                PathVerb::Move => out.move_to(kp(take(1)?[0])),
                PathVerb::Line => out.line_to(kp(take(1)?[0])),
                PathVerb::Quad => {
                    let p = take(2)?;
                    out.quad_to(kp(p[0]), kp(p[1]));
                }
                PathVerb::Cubic => {
                    let p = take(3)?;
                    out.curve_to(kp(p[0]), kp(p[1]), kp(p[2]));
                }
                PathVerb::Conic => return Err("conic in output".to_owned()),
                PathVerb::Close => out.close_path(),
            }
        }
        Ok(out)
    }
}

#[cfg(feature = "sweep")]
mod sweep {
    use kurbo::BezPath;
    use linesweeper::{BinaryOp, FillRule, binary_op};

    /// Union with an empty set, as in fontc PR #2070.
    pub fn union(path: &BezPath) -> Result<BezPath, String> {
        let contours = binary_op(path, &BezPath::new(), FillRule::NonZero, BinaryOp::Union)
            .map_err(|e| e.to_string())?;
        Ok(contours.contours().flat_map(|c| c.path.iter()).collect())
    }
}
