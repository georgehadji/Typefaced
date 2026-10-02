//! The case set: overlapping corpus glyphs, the test fixtures, and synthetic
//! edge cases. Corpus glyphs are selected by a rule, not by hand: every glyph
//! of every master whose own contours overlap (|w| >= 2 somewhere) or mix
//! directions (both w = +1 and w = -1 regions).

use std::path::{Path, PathBuf};

use kurbo::{BezPath, Circle, Point, Rect, Shape};

use crate::measure;

/// Minimum overlap or mixed-winding area, in square units, for a corpus glyph
/// to count as overlapping. Filters out contours that only touch.
const OVERLAP_THRESHOLD: f64 = 0.5;

pub struct Case {
    pub group: &'static str,
    pub name: String,
    pub path: BezPath,
}

/// Source Sans 3 masters, from `SourceSans3VF-Upright.designspace`.
const SOURCE_SANS_MASTERS: &[&str] = &[
    "Poles/pole_0/SourceSans3-ExtraLight.ufo",
    "Poles/pole_1/SourceSans3-Upright.ufo",
    "Poles/pole_2/SourceSans3-Black.ufo",
];

/// Default location of `tests/` (the corpus cache and the fixtures).
pub fn default_tests_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests")
}

/// All cases: overlapping glyphs of both corpus families, every fixture glyph
/// with contours, and the synthetic set.
pub fn all(tests: &Path) -> Result<Vec<Case>, String> {
    let cache = tests.join("corpus/.cache");
    if !cache.is_dir() {
        return Err(format!(
            "{} is missing: run `cargo xtask corpus fetch` first",
            cache.display()
        ));
    }
    let source_sans: Vec<PathBuf> = SOURCE_SANS_MASTERS
        .iter()
        .map(|m| cache.join("source-sans-upright").join(m))
        .collect();
    let mut inria: Vec<PathBuf> = std::fs::read_dir(cache.join("inria-sans"))
        .map_err(|e| format!("inria-sans: {e}"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "ufo"))
        .collect();
    inria.sort();

    let mut cases = Vec::new();
    for ufo in &source_sans {
        cases.extend(load_ufo("source-sans", ufo, true)?);
    }
    for ufo in &inria {
        cases.extend(load_ufo("inria-sans", ufo, true)?);
    }
    cases.extend(load_fixture_glyphs(&tests.join("fixtures/min.ufo"))?);
    cases.extend(synthetic());
    Ok(cases)
}

/// Whether a path's own contours overlap or mix directions.
pub fn overlaps(path: &BezPath) -> bool {
    let p = measure::compare(&BezPath::new(), path);
    p.overlap_area > OVERLAP_THRESHOLD || p.mixed_winding_area() > OVERLAP_THRESHOLD
}

fn error_chain(e: &dyn std::error::Error) -> String {
    let mut msg = e.to_string();
    let mut source = e.source();
    while let Some(s) = source {
        msg.push_str(": ");
        msg.push_str(&s.to_string());
        source = s.source();
    }
    msg
}

fn load_ufo(group: &'static str, ufo: &Path, only_overlapping: bool) -> Result<Vec<Case>, String> {
    let font =
        norad::Font::load(ufo).map_err(|e| format!("{}: {}", ufo.display(), error_chain(&e)))?;
    let glyphs = font.default_layer().iter().cloned().collect();
    glyph_cases(group, ufo, glyphs, only_overlapping)
}

/// norad 0.18.4 rejects XML comments inside `<outline>`, which the fixtures use,
/// so their glyphs are parsed one by one with the comments stripped.
fn load_fixture_glyphs(ufo: &Path) -> Result<Vec<Case>, String> {
    let dir = ufo.join("glyphs");
    let mut glyphs = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().is_none_or(|x| x != "glif") {
            continue;
        }
        let mut xml =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        while let Some(start) = xml.find("<!--") {
            let end = xml[start..]
                .find("-->")
                .map_or(xml.len(), |i| start + i + 3);
            xml.replace_range(start..end, "");
        }
        glyphs.push(
            norad::Glyph::parse_raw(xml.as_bytes())
                .map_err(|e| format!("{}: {}", path.display(), error_chain(&e)))?,
        );
    }
    glyph_cases("fixtures", ufo, glyphs, false)
}

/// The closed contours of each glyph, sorted by name. Components are not
/// decomposed and open contours are skipped.
fn glyph_cases(
    group: &'static str,
    ufo: &Path,
    mut glyphs: Vec<norad::Glyph>,
    only_overlapping: bool,
) -> Result<Vec<Case>, String> {
    let stem = ufo
        .file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    glyphs.sort_by(|a, b| a.name().cmp(b.name()));
    let (mut cases, mut with_contours, mut open) = (Vec::new(), 0, 0);
    for glyph in &glyphs {
        let mut path = BezPath::new();
        open += glyph.contours.iter().filter(|c| !c.is_closed()).count();
        for contour in glyph.contours.iter().filter(|c| c.is_closed()) {
            let mut c = contour
                .to_kurbo()
                .map_err(|e| format!("{stem}/{}: {e}", glyph.name()))?;
            c.close_path();
            path.extend(c);
        }
        if path.is_empty() {
            continue;
        }
        with_contours += 1;
        if only_overlapping && !overlaps(&path) {
            continue;
        }
        cases.push(Case {
            group,
            name: format!("{stem}/{}", glyph.name()),
            path,
        });
    }
    eprintln!(
        "{group} {stem}: {} glyphs, {with_contours} with closed contours, {} selected, {open} open contours skipped",
        glyphs.len(),
        cases.len()
    );
    Ok(cases)
}

fn oriented(path: BezPath, counter_clockwise: bool) -> BezPath {
    if (path.area() > 0.0) == counter_clockwise {
        path
    } else {
        path.reverse_subpaths()
    }
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> BezPath {
    oriented(Rect::new(x0, y0, x1, y1).to_path(0.0), true)
}

fn circle(cx: f64, cy: f64, r: f64, counter_clockwise: bool) -> BezPath {
    // Tolerance 0.1 gives four cubic arcs, as in a drawn font outline.
    oriented(Circle::new((cx, cy), r).to_path(0.1), counter_clockwise)
}

fn polygon(points: &[(f64, f64)]) -> BezPath {
    let mut path = BezPath::new();
    path.move_to(points[0]);
    for &p in &points[1..] {
        path.line_to(p);
    }
    path.close_path();
    path
}

fn join(parts: impl IntoIterator<Item = BezPath>) -> BezPath {
    parts.into_iter().flat_map(|p| p.into_iter()).collect()
}

/// Deterministic jitter in [-1, 1) (a 64-bit LCG), so runs are repeatable.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 52) as f64 - 1.0
    }
}

fn ellipse_o(dx: f64) -> BezPath {
    // kurbo's ellipse path is an open arc; fonts close their contours.
    let ellipse = |rx: f64, ry: f64| {
        let mut p = kurbo::Ellipse::new((320.0 + dx, 350.0), (rx, ry), 0.0).to_path(0.1);
        p.close_path();
        p
    };
    join([
        oriented(ellipse(280.0, 360.0), true),
        oriented(ellipse(170.0, 250.0), false),
    ])
}

/// Synthetic edge cases in a 1,000-unit em.
pub fn synthetic() -> Vec<Case> {
    let mut rng = Lcg(0x5EED);
    let tiny_circles = join((0..200).map(|i| {
        let (gx, gy) = (f64::from(i % 20), f64::from(i / 20));
        circle(
            gx * 12.0 + rng.next() * 3.0,
            gy * 12.0 + rng.next() * 3.0,
            8.0,
            true,
        )
    }));
    let tiny_squares = join((0..200).map(|i| {
        let (gx, gy) = (f64::from(i % 20), f64::from(i / 20));
        let c = Point::new(gx * 8.0, gy * 8.0);
        let a = rng.next() * std::f64::consts::PI;
        let corners: Vec<(f64, f64)> = (0..4)
            .map(|k| {
                let t = a + f64::from(k) * std::f64::consts::FRAC_PI_2;
                (c.x + 7.0 * t.cos(), c.y + 7.0 * t.sin())
            })
            .collect();
        oriented(polygon(&corners), true)
    }));
    let star: Vec<(f64, f64)> = (0..5)
        .map(|k| {
            let t = std::f64::consts::FRAC_PI_2 + f64::from(k) * 4.0 * std::f64::consts::PI / 5.0;
            (300.0 * t.cos(), 300.0 * t.sin())
        })
        .collect();
    let mut figure_eight = BezPath::new();
    figure_eight.move_to((0.0, 0.0));
    figure_eight.curve_to((100.0, 200.0), (300.0, -200.0), (400.0, 0.0));
    figure_eight.curve_to((300.0, 200.0), (100.0, -200.0), (0.0, 0.0));
    figure_eight.close_path();
    let mut cubic_loop = BezPath::new();
    cubic_loop.move_to((0.0, 0.0));
    cubic_loop.curve_to((300.0, 300.0), (0.0, 300.0), (300.0, 0.0));
    cubic_loop.close_path();
    let circle_twice = {
        let c = circle(0.0, 0.0, 200.0, true);
        let body: Vec<_> = c
            .iter()
            .filter(|e| !matches!(e, kurbo::PathEl::ClosePath))
            .collect();
        let mut p: BezPath = body.iter().copied().collect();
        p.extend(body.iter().copied().skip(1));
        p.close_path();
        p
    };

    let cases: Vec<(&str, BezPath)> = vec![
        (
            "coincident-shared-edge",
            join([rect(0.0, 0.0, 100.0, 100.0), rect(100.0, 0.0, 200.0, 100.0)]),
        ),
        (
            "coincident-partial-edge",
            join([rect(0.0, 0.0, 100.0, 100.0), rect(50.0, 0.0, 150.0, 60.0)]),
        ),
        (
            "coincident-duplicate",
            join([rect(0.0, 0.0, 100.0, 100.0), rect(0.0, 0.0, 100.0, 100.0)]),
        ),
        (
            "coincident-reversed-duplicate",
            join([
                rect(0.0, 0.0, 100.0, 100.0),
                rect(0.0, 0.0, 100.0, 100.0).reverse_subpaths(),
            ]),
        ),
        (
            "coincident-curves",
            join([circle(0.0, 0.0, 200.0, true), circle(0.0, 0.0, 200.0, true)]),
        ),
        (
            "coincident-collinear-50",
            join(
                (0..50).map(|i| rect(f64::from(i) * 10.0, 0.0, f64::from(i) * 10.0 + 25.0, 100.0)),
            ),
        ),
        (
            "near-coincident-1e-4",
            join([
                rect(0.0, 0.0, 100.0, 100.0),
                rect(1e-4, 50.0, 100.0 + 1e-4, 150.0),
            ]),
        ),
        (
            "tangent-circles-outside",
            join([
                circle(0.0, 0.0, 100.0, true),
                circle(200.0, 0.0, 100.0, true),
            ]),
        ),
        (
            "tangent-circles-inside",
            join([circle(0.0, 0.0, 100.0, true), circle(50.0, 0.0, 50.0, true)]),
        ),
        (
            "tangent-counter",
            join([
                circle(0.0, 0.0, 100.0, true),
                circle(50.0, 0.0, 50.0, false),
            ]),
        ),
        (
            "tangent-circle-square",
            join([
                circle(0.0, 0.0, 100.0, true),
                rect(-50.0, 100.0, 50.0, 200.0),
            ]),
        ),
        (
            "sliver-overlap-0.005",
            join([
                rect(0.0, 0.0, 100.0, 300.0),
                rect(99.995, 0.0, 200.0, 300.0),
            ]),
        ),
        (
            "sliver-gap-0.005",
            join([
                rect(0.0, 0.0, 100.0, 300.0),
                rect(100.005, 0.0, 200.0, 300.0),
            ]),
        ),
        (
            "sliver-contour-0.007",
            join([
                oriented(
                    polygon(&[(0.0, 0.0), (300.0, 100.0), (300.0, 100.007), (0.0, 0.007)]),
                    true,
                ),
                rect(100.0, -50.0, 200.0, 150.0),
            ]),
        ),
        (
            "sliver-crescent-0.005",
            join([
                circle(0.0, 0.0, 200.0, true),
                circle(0.005, 0.0, 200.0, true),
            ]),
        ),
        (
            "figure-eight-lines",
            polygon(&[(0.0, 0.0), (200.0, 200.0), (200.0, 0.0), (0.0, 200.0)]),
        ),
        ("figure-eight-cubic", figure_eight),
        ("cubic-self-loop", cubic_loop),
        ("pentagram", polygon(&star)),
        ("circle-traced-twice", circle_twice),
        ("tiny-circles-200", tiny_circles),
        ("tiny-squares-200", tiny_squares),
        (
            "nested-counters-alternating",
            join([
                circle(0.0, 0.0, 300.0, true),
                circle(0.0, 0.0, 200.0, false),
                circle(0.0, 0.0, 100.0, true),
            ]),
        ),
        (
            "nested-same-direction",
            join([
                circle(0.0, 0.0, 300.0, true),
                circle(0.0, 0.0, 200.0, true),
                circle(0.0, 0.0, 100.0, true),
            ]),
        ),
        (
            "nested-overlapping-o",
            join([ellipse_o(0.0), ellipse_o(250.0)]),
        ),
        (
            "nested-counter-slash",
            join([
                rect(0.0, 0.0, 600.0, 700.0),
                oriented(rect(100.0, 100.0, 500.0, 600.0), false),
                oriented(
                    polygon(&[
                        (30.0, -50.0),
                        (110.0, -50.0),
                        (570.0, 750.0),
                        (490.0, 750.0),
                    ]),
                    true,
                ),
            ]),
        ),
        (
            "far-from-origin-30000",
            join([
                circle(30_000.0, 30_000.0, 100.0, true),
                circle(30_100.0, 30_000.0, 100.0, true),
            ]),
        ),
        (
            "zero-width-spike",
            polygon(&[
                (0.0, 0.0),
                (200.0, 0.0),
                (200.0, 200.0),
                (100.0, 200.0),
                (100.0, 400.0),
                (100.0, 200.0),
                (0.0, 200.0),
            ]),
        ),
        ("empty", BezPath::new()),
    ];
    cases
        .into_iter()
        .map(|(name, path)| Case {
            group: "synthetic",
            name: name.to_owned(),
            path,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_overlap_cases_overlap() {
        let cases = synthetic();
        let get = |n: &str| &cases.iter().find(|c| c.name == n).expect(n).path;
        for n in [
            "coincident-duplicate",
            "tiny-circles-200",
            "pentagram",
            "nested-same-direction",
            "circle-traced-twice",
        ] {
            assert!(overlaps(get(n)), "{n} should overlap");
        }
        for n in [
            "coincident-shared-edge",
            "tangent-circles-outside",
            "nested-counters-alternating",
            "empty",
        ] {
            assert!(!overlaps(get(n)), "{n} should not overlap");
        }
        // Lobes of a figure-eight wind in opposite directions.
        assert!(overlaps(get("figure-eight-cubic")));
    }
}
