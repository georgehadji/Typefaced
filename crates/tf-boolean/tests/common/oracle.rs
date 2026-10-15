//! The engine-independent scanline oracle, ported from Spike 5 (`spikes/spike-boolean/src/measure.rs`).
//!
//! Both paths are flattened with kurbo and integrated along horizontal scanlines: on each
//! scanline the winding number is exact between edge crossings, so the only errors are the
//! flattening tolerance and the vertical step. [`compare`] scans the whole case (8,192
//! scanlines over the union bounding box, tolerance 0.001 units). [`coverage`] scans only
//! the bounding box of one small feature, with a finer tolerance, so it can see features
//! far thinner than the whole-case step (the thin-feature check ADR-0008 asks for).

use kurbo::{BezPath, PathEl, Point, Shape};

/// Flattening tolerance of [`compare`], in font units.
const FLATTEN_TOLERANCE: f64 = 1e-3;
/// Flattening tolerance of [`coverage`]: the sliver features are 0.004–0.007 units wide.
const FINE_TOLERANCE: f64 = 1e-6;
/// Scanlines per scan.
const SCANLINES: usize = 8192;

/// Areas in square font units. Winding numbers are positive for counter-clockwise
/// contours in a y-up coordinate system (PostScript direction).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Comparison {
    /// Area of the reference under the non-zero rule.
    pub reference_area: f64,
    /// Area of the output under the non-zero rule.
    pub output_area: f64,
    /// Area inside both reference and output.
    pub both_area: f64,
    /// Symmetric difference between reference and output (both non-zero).
    pub xor_area: f64,
    /// Output area whose winding number has |w| >= 2: overlaps left behind.
    pub overlap_area: f64,
    /// Output area with w >= 1.
    pub positive_area: f64,
    /// Output area with w <= -1.
    pub negative_area: f64,
}

impl Comparison {
    /// The smaller of the positive and negative regions: non-zero means the output mixes
    /// contour directions for filled regions.
    pub fn mixed_winding_area(&self) -> f64 {
        self.positive_area.min(self.negative_area)
    }

    /// `area` relative to the reference area, floored at 1 square unit so that empty
    /// references (cancelling contours) report the absolute value.
    pub fn relative(&self, area: f64) -> f64 {
        area / self.reference_area.max(1.0)
    }
}

struct Edge {
    y0: f64,
    y1: f64,
    x0: f64,
    slope: f64,
    d_ref: i32,
    d_out: i32,
}

#[derive(Default)]
struct EdgeBuilder {
    edges: Vec<Edge>,
    start: Option<Point>,
    last: Point,
    tag_output: bool,
}

impl EdgeBuilder {
    fn line(&mut self, a: Point, b: Point) {
        if a.y == b.y {
            return;
        }
        // Downward edges count +1, so counter-clockwise (y-up) contours have w = +1.
        let (lo, hi, dir) = if a.y < b.y { (a, b, -1) } else { (b, a, 1) };
        let (d_ref, d_out) = if self.tag_output { (0, dir) } else { (dir, 0) };
        self.edges.push(Edge {
            y0: lo.y,
            y1: hi.y,
            x0: lo.x,
            slope: (hi.x - lo.x) / (hi.y - lo.y),
            d_ref,
            d_out,
        });
    }

    /// Fill semantics close every subpath, explicitly or not.
    fn close(&mut self) {
        if let Some(start) = self.start.take() {
            self.line(self.last, start);
            self.last = start;
        }
    }

    fn add(&mut self, path: &BezPath, tag_output: bool, tolerance: f64) {
        self.tag_output = tag_output;
        kurbo::flatten(path.iter(), tolerance, |el| match el {
            PathEl::MoveTo(p) => {
                self.close();
                self.start = Some(p);
                self.last = p;
            }
            PathEl::LineTo(p) => {
                self.line(self.last, p);
                self.last = p;
            }
            PathEl::ClosePath => self.close(),
            // flatten emits only MoveTo, LineTo and ClosePath.
            PathEl::QuadTo(..) | PathEl::CurveTo(..) => {}
        });
        self.close();
    }
}

/// Compares `output` against `reference` over their union bounding box. Pass an empty
/// reference to profile the windings of a single path. Both paths must be finite
/// (flattening a non-finite path does not terminate): check [`structure`] first.
pub fn compare(reference: &BezPath, output: &BezPath) -> Comparison {
    scan(reference, output, FLATTEN_TOLERANCE, None)
}

/// Compares `output` against a small `feature` inside the feature's own vertical extent:
/// `both_area` is the part of the feature that the output covers.
pub fn coverage(feature: &BezPath, output: &BezPath) -> Comparison {
    let bbox = feature.bounding_box();
    scan(feature, output, FINE_TOLERANCE, Some((bbox.y0, bbox.y1)))
}

fn scan(
    reference: &BezPath,
    output: &BezPath,
    tolerance: f64,
    y_range: Option<(f64, f64)>,
) -> Comparison {
    let mut builder = EdgeBuilder::default();
    builder.add(reference, false, tolerance);
    builder.add(output, true, tolerance);
    let mut edges = builder.edges;
    let mut result = Comparison::default();
    if edges.is_empty() {
        return result;
    }
    edges.sort_by(|a, b| a.y0.total_cmp(&b.y0));
    let (y_min, y_max) = y_range.unwrap_or_else(|| {
        let top = edges.iter().map(|e| e.y1).fold(f64::NEG_INFINITY, f64::max);
        (edges[0].y0, top)
    });
    let step = (y_max - y_min) / SCANLINES as f64;

    let mut next = 0;
    let mut active: Vec<&Edge> = Vec::new();
    let mut crossings: Vec<(f64, i32, i32)> = Vec::new();
    for k in 0..SCANLINES {
        let y = y_min + (k as f64 + 0.5) * step;
        while next < edges.len() && edges[next].y0 <= y {
            active.push(&edges[next]);
            next += 1;
        }
        active.retain(|e| e.y1 > y);
        crossings.clear();
        crossings.extend(
            active
                .iter()
                .map(|e| (e.x0 + (y - e.y0) * e.slope, e.d_ref, e.d_out)),
        );
        crossings.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (mut w_ref, mut w_out) = (0, 0);
        for pair in crossings.windows(2) {
            w_ref += pair[0].1;
            w_out += pair[0].2;
            let len = (pair[1].0 - pair[0].0) * step;
            let (in_ref, in_out) = (w_ref != 0, w_out != 0);
            if in_ref {
                result.reference_area += len;
            }
            if in_out {
                result.output_area += len;
            }
            if in_ref && in_out {
                result.both_area += len;
            }
            if in_ref != in_out {
                result.xor_area += len;
            }
            if w_out.abs() >= 2 {
                result.overlap_area += len;
            }
            if w_out >= 1 {
                result.positive_area += len;
            }
            if w_out <= -1 {
                result.negative_area += len;
            }
        }
    }
    result
}

/// Structural checks on an engine's output.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Structure {
    pub contours: usize,
    /// Subpaths that neither end with ClosePath nor return to their start point.
    pub open_contours: usize,
    /// Whether any coordinate is NaN or infinite.
    pub non_finite: bool,
    /// Contours enclosing less than [`TINY_CONTOUR_AREA`].
    pub tiny_contours: usize,
}

/// Square font units.
pub const TINY_CONTOUR_AREA: f64 = 1.0;

/// The subpaths of `path`, one per contour.
pub fn subpaths(path: &BezPath) -> Vec<BezPath> {
    let mut subpaths: Vec<BezPath> = Vec::new();
    for el in path.iter() {
        if matches!(el, PathEl::MoveTo(_)) || subpaths.is_empty() {
            subpaths.push(BezPath::new());
        }
        if let Some(sub) = subpaths.last_mut() {
            sub.push(el);
        }
    }
    subpaths
}

pub fn structure(path: &BezPath) -> Structure {
    let subpaths = subpaths(path);
    let mut s = Structure {
        contours: subpaths.len(),
        ..Structure::default()
    };
    for sub in &subpaths {
        let els = sub.elements();
        let start = els.first().and_then(PathEl::end_point);
        let end = els.iter().rev().find_map(PathEl::end_point);
        let closed = matches!(els.last(), Some(PathEl::ClosePath))
            || matches!((start, end), (Some(a), Some(b)) if a.distance(b) <= 1e-9);
        s.open_contours += usize::from(!closed);
        s.non_finite |= els.iter().any(|el| match *el {
            PathEl::MoveTo(p) | PathEl::LineTo(p) => !p.is_finite(),
            PathEl::QuadTo(a, p) => !a.is_finite() || !p.is_finite(),
            PathEl::CurveTo(a, b, p) => !a.is_finite() || !b.is_finite() || !p.is_finite(),
            PathEl::ClosePath => false,
        });
        s.tiny_contours += usize::from(sub.area().abs() < TINY_CONTOUR_AREA);
    }
    s
}
