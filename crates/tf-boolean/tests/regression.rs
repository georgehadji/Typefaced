//! The regression corpus of ADR-0008: remove-overlap on the 581 cases of Spike 5, checked
//! against the scanline oracle, plus the thin-feature and debris checks.
//!
//! The 29 synthetic and 4 fixture cases run in every `cargo test`. The 548 Source Sans
//! cases need the corpus and run with `cargo xtask corpus fetch` and
//! `cargo test -p tf-boolean --locked -- --ignored --nocapture`: the upgrade check that
//! every linesweeper upgrade reruns.

// Each test target uses only part of the shared module.
#[allow(dead_code)]
mod common;

use std::panic::{AssertUnwindSafe, catch_unwind};

use common::cases::{self, Case};
use common::oracle;
use kurbo::{Affine, BezPath};
use tf_boolean::Linesweeper;
use tf_geometry::BooleanEngine;

/// Overlap left, clockwise fill and area error fail above this fraction of the input area.
const AREA_TOLERANCE: f64 = 1e-3;

/// What the whole run found; every case runs even when an earlier one fails.
#[derive(Default)]
struct Summary {
    cases: usize,
    crashes: usize,
    errors: usize,
    failures: Vec<String>,
    worst_error: f64,
    worst_case: String,
}

impl Summary {
    fn run(cases: &[Case]) -> Self {
        let mut s = Summary::default();
        for case in cases {
            s.cases += 1;
            let name = format!("{}/{}", case.group, case.name);
            let out =
                match catch_unwind(AssertUnwindSafe(|| Linesweeper.remove_overlap(&case.path))) {
                    Err(_) => {
                        s.crashes += 1;
                        s.failures.push(format!("{name}: panic"));
                        continue;
                    }
                    Ok(Err(e)) => {
                        s.errors += 1;
                        s.failures.push(format!("{name}: error: {e}"));
                        continue;
                    }
                    Ok(Ok(out)) => out,
                };
            let (problems, error) = check(&case.path, &out);
            if s.worst_case.is_empty() || error.is_nan() || error > s.worst_error {
                s.worst_error = error;
                s.worst_case.clone_from(&name);
            }
            if !problems.is_empty() {
                s.failures.push(format!("{name}: {}", problems.join(", ")));
            }
        }
        s
    }

    fn assert_clean(&self, expected_cases: usize) {
        println!(
            "{} cases: {} crashes, {} errors, {} failing, worst area error {:.5}% ({})",
            self.cases,
            self.crashes,
            self.errors,
            self.failures.len(),
            100.0 * self.worst_error,
            self.worst_case,
        );
        assert!(self.failures.is_empty(), "{}", self.failures.join("\n"));
        assert_eq!(self.cases, expected_cases, "the case selection changed");
    }
}

/// The problems of one output, and its area error (NaN when the oracle cannot run).
fn check(input: &BezPath, out: &BezPath) -> (Vec<String>, f64) {
    let structure = oracle::structure(out);
    let mut p = Vec::new();
    if structure.open_contours > 0 {
        p.push(format!("{} open contours", structure.open_contours));
    }
    // Flattening a non-finite path does not terminate: skip the oracle.
    if structure.non_finite {
        p.push("non-finite coordinates".to_owned());
        return (p, f64::NAN);
    }
    let c = oracle::compare(input, out);
    for (what, area) in [
        ("area error", c.xor_area),
        ("overlap left", c.overlap_area),
        // Filled regions wound clockwise: mixed directions, or every contour reversed.
        ("clockwise fill", c.negative_area),
    ] {
        let rel = c.relative(area);
        if rel.is_nan() || rel > AREA_TOLERANCE {
            p.push(format!("{what} {:.4}%", 100.0 * rel));
        }
    }
    (p, c.relative(c.xor_area))
}

fn synthetic_case(name: &str) -> BezPath {
    cases::synthetic()
        .into_iter()
        .find(|c| c.name == name)
        .map(|c| c.path)
        .unwrap_or_else(|| panic!("no synthetic case {name}"))
}

#[test]
fn oracle_measures_known_shapes() {
    use cases::{circle, rect};
    let c = oracle::compare(&rect(0.0, 0.0, 100.0, 100.0), &BezPath::new());
    assert!((c.reference_area - 10_000.0).abs() < 1e-6, "{c:?}");
    // Counter-clockwise (y-up) winds +1.
    let c = oracle::compare(&BezPath::new(), &rect(0.0, 0.0, 100.0, 100.0));
    assert!((c.positive_area - 10_000.0).abs() < 1e-6, "{c:?}");
    assert_eq!(c.negative_area, 0.0);
    let circle = circle(0.0, 0.0, 300.0, true);
    let c = oracle::compare(&circle, &circle);
    let exact = std::f64::consts::PI * 300.0 * 300.0;
    assert!((c.reference_area - exact).abs() / exact < 1e-3, "{c:?}");
    assert_eq!(c.xor_area, 0.0);
    let two = cases::join([rect(0.0, 0.0, 100.0, 100.0), rect(50.0, 0.0, 150.0, 100.0)]);
    let profile = oracle::compare(&BezPath::new(), &two);
    assert!((profile.overlap_area - 5_000.0).abs() < 1e-6, "{profile:?}");
    let mixed = cases::join([
        rect(0.0, 0.0, 100.0, 100.0),
        rect(200.0, 0.0, 300.0, 100.0).reverse_subpaths(),
    ]);
    let c = oracle::compare(&BezPath::new(), &mixed);
    assert!((c.mixed_winding_area() - 10_000.0).abs() < 1e-6, "{c:?}");
    // The local scan resolves a strip far thinner than the whole-case step.
    let strip = rect(10.0, 0.0, 10.004, 0.02);
    let c = oracle::coverage(&strip, &rect(0.0, 0.0, 10.002, 300.0));
    assert!((c.both_area / c.reference_area - 0.5).abs() < 1e-3, "{c:?}");
    let s = oracle::structure(&two);
    assert_eq!((s.contours, s.open_contours, s.non_finite), (2, 0, false));
}

#[test]
fn synthetic_cases_pass() {
    let cases = cases::synthetic();
    for (name, overlapping) in [
        ("coincident-duplicate", true),
        ("tiny-circles-200", true),
        ("pentagram", true),
        ("figure-eight-cubic", true),
        ("coincident-shared-edge", false),
        ("nested-counters-alternating", false),
        ("empty", false),
    ] {
        assert_eq!(
            cases::overlaps(&synthetic_case(name)),
            overlapping,
            "{name}"
        );
    }
    Summary::run(&cases).assert_clean(29);
}

#[test]
fn fixture_glyphs_pass() {
    let cases = cases::fixtures().unwrap_or_else(|e| panic!("{e}"));
    let mut names: Vec<&str> = cases.iter().map(|c| c.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["min/.notdef", "min/A", "min/O", "min/acute"]);
    Summary::run(&cases).assert_clean(4);
}

#[test]
#[ignore = "needs the corpus: cargo xtask corpus fetch"]
fn source_sans_corpus_passes() {
    let cases = cases::corpus().unwrap_or_else(|e| panic!("{e}"));
    Summary::run(&cases).assert_clean(548);
}

/// The inside of `sliver-contour-0.007` (the strip y = x / 3 to y = x / 3 + 0.007) between
/// `x0` and `x1`, 0.0005 units in from each long edge.
fn sliver_part(x0: f64, x1: f64) -> BezPath {
    cases::polygon(&[
        (x0, x0 / 3.0 + 0.0005),
        (x1, x1 / 3.0 + 0.0005),
        (x1, x1 / 3.0 + 0.0065),
        (x0, x0 / 3.0 + 0.0065),
    ])
}

/// Thin features (ADR-0008): each sliver is measured on its own, so a sliver deleted or
/// filled by the engine shows up even though it is far below 0.1% of the whole case.
#[test]
fn slivers_survive() {
    use cases::rect;
    // (case, feature, whether the output must cover the feature)
    let checks: [(&str, BezPath, bool); 4] = [
        // The 0.005-wide overlap of two rectangles is filled.
        (
            "sliver-overlap-0.005",
            rect(99.9955, 1.0, 99.9995, 299.0),
            true,
        ),
        // The 0.005-wide gap between two rectangles stays empty.
        (
            "sliver-gap-0.005",
            rect(100.0005, 1.0, 100.0045, 299.0),
            false,
        ),
        // The 0.007-thick slanted contour survives on both sides of the rectangle it crosses.
        (
            "sliver-contour-0.007",
            cases::join([sliver_part(1.0, 99.0), sliver_part(201.0, 299.0)]),
            true,
        ),
        // The 0.005-wide crescent where the shifted circle sticks out, at its widest.
        (
            "sliver-crescent-0.005",
            rect(200.0005, -0.01, 200.0045, 0.01),
            true,
        ),
    ];
    for (name, feature, covered) in checks {
        let out = Linesweeper
            .remove_overlap(&synthetic_case(name))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let c = oracle::coverage(&feature, &out);
        let fraction = c.both_area / c.reference_area;
        assert!(c.reference_area > 0.0, "{name}: empty feature");
        if covered {
            assert!(
                fraction > 0.999,
                "{name}: only {fraction} of the sliver covered"
            );
        } else {
            assert!(fraction < 0.001, "{name}: {fraction} of the gap filled");
        }
    }
}

/// The debris filter: coincident curves leave a contour under 1e-7 unit² (Spike 5), which
/// is dropped, while the real small holes between tiny overlapping shapes stay.
#[test]
fn debris_is_dropped_and_small_holes_stay() {
    for name in ["coincident-curves", "circle-traced-twice"] {
        let out = Linesweeper.remove_overlap(&synthetic_case(name)).unwrap();
        assert_eq!(oracle::structure(&out).contours, 1, "{name}");
    }
    for (name, holes) in [("tiny-circles-200", 25), ("tiny-squares-200", 92)] {
        let out = Linesweeper.remove_overlap(&synthetic_case(name)).unwrap();
        assert_eq!(oracle::structure(&out).tiny_contours, holes, "{name}");
    }
}

/// The threshold was measured on cases about 1,000 units wide. At ten times that size (a
/// 10,000-unit em, or an outline drawn large) the debris still drops and every contour of
/// the small-hole cases stays.
#[test]
fn debris_threshold_holds_at_ten_times_the_size() {
    let large = |name: &str| Affine::scale(10.0) * synthetic_case(name);
    for name in ["coincident-curves", "circle-traced-twice"] {
        let out = Linesweeper.remove_overlap(&large(name)).unwrap();
        assert_eq!(oracle::structure(&out).contours, 1, "{name}");
    }
    for name in ["tiny-circles-200", "tiny-squares-200"] {
        let contours =
            |path: &BezPath| oracle::structure(&Linesweeper.remove_overlap(path).unwrap()).contours;
        assert_eq!(
            contours(&large(name)),
            contours(&synthetic_case(name)),
            "{name}"
        );
    }
}
