//! Engine-independent reference measurements.
//!
//! Both paths are flattened with kurbo (tolerance 0.001 units) and integrated
//! along horizontal scanlines: on each scanline the winding number is exact
//! between edge crossings, so the only errors are the flattening tolerance and
//! the vertical sampling step (at most 0.125 units for a 1,000-unit glyph).
//! This is a rasteriser with exact horizontal coverage; a pixel rasteriser
//! (tiny-skia, 4x4 supersampling) would add about 1/16 pixel of noise per
//! boundary pixel, which is close to the 0.1% threshold on thin glyphs.

use kurbo::{BezPath, PathEl, Point, Shape};

/// Flattening tolerance in font units.
const FLATTEN_TOLERANCE: f64 = 1e-3;
/// Scanlines per case: 8,192 over the union bounding box.
const SCANLINES: usize = 8192;

/// Areas in square font units. Winding numbers are positive for
/// counter-clockwise contours in a y-up coordinate system (PostScript direction).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Comparison {
    /// Area of the reference under the non-zero rule.
    pub reference_area: f64,
    /// Area of the output under the non-zero rule.
    pub output_area: f64,
    /// Symmetric difference between reference and output (both non-zero).
    pub xor_area: f64,
    /// The same with the output read under the even-odd rule: separates wrong
    /// geometry from wrong contour directions.
    pub xor_even_odd_area: f64,
    /// Output area whose winding number has |w| >= 2: overlaps left behind.
    pub overlap_area: f64,
    /// Output area with w >= 1.
    pub positive_area: f64,
    /// Output area with w <= -1.
    pub negative_area: f64,
}

impl Comparison {
    /// Area of the smaller of the positive and negative regions: non-zero means
    /// the output mixes contour directions for filled regions.
    pub fn mixed_winding_area(&self) -> f64 {
        self.positive_area.min(self.negative_area)
    }

    /// Symmetric difference relative to the reference area. The denominator is
    /// floored at 1 square unit so that empty references (cancelling contours)
    /// report the absolute difference.
    pub fn area_error(&self) -> f64 {
        self.xor_area / self.reference_area.max(1.0)
    }

    /// [`Self::area_error`] with the output read under the even-odd rule.
    pub fn area_error_even_odd(&self) -> f64 {
        self.xor_even_odd_area / self.reference_area.max(1.0)
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

    fn add(&mut self, path: &BezPath, tag_output: bool) {
        self.tag_output = tag_output;
        kurbo::flatten(path.iter(), FLATTEN_TOLERANCE, |el| match el {
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

/// Compares `output` against `reference`. Pass an empty reference to profile
/// the windings of a single path.
pub fn compare(reference: &BezPath, output: &BezPath) -> Comparison {
    let mut builder = EdgeBuilder::default();
    builder.add(reference, false);
    builder.add(output, true);
    let mut edges = builder.edges;
    let mut result = Comparison::default();
    if edges.is_empty() {
        return result;
    }
    edges.sort_by(|a, b| a.y0.total_cmp(&b.y0));
    let y_min = edges[0].y0;
    let y_max = edges.iter().map(|e| e.y1).fold(f64::NEG_INFINITY, f64::max);
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
            if in_ref != in_out {
                result.xor_area += len;
            }
            if in_ref != (w_out % 2 != 0) {
                result.xor_even_odd_area += len;
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
    /// Contours enclosing less than [`TINY_CONTOUR_AREA`]: debris a font
    /// would have to clean up.
    pub tiny_contours: usize,
    /// Lines and curves (closing lines not counted).
    pub segments: usize,
}

/// Square font units.
pub const TINY_CONTOUR_AREA: f64 = 1.0;

pub fn structure(path: &BezPath) -> Structure {
    let mut subpaths: Vec<BezPath> = Vec::new();
    for el in path.iter() {
        if matches!(el, PathEl::MoveTo(_)) || subpaths.is_empty() {
            subpaths.push(BezPath::new());
        }
        if let Some(sub) = subpaths.last_mut() {
            sub.push(el);
        }
    }
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
        // An explicit line back to the start before ClosePath is the implicit
        // closing edge; engines differ in whether they emit it, so it is not counted.
        let explicit_close = matches!(
            (els.iter().rev().find(|el| !matches!(el, PathEl::ClosePath)), start),
            (Some(PathEl::LineTo(p)), Some(s0)) if *p == s0
        );
        s.segments += els
            .iter()
            .filter(|el| !matches!(el, PathEl::MoveTo(_) | PathEl::ClosePath))
            .count()
            - usize::from(explicit_close);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::{Circle, Rect};

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> BezPath {
        Rect::new(x0, y0, x1, y1).to_path(0.0)
    }

    #[test]
    fn square_area_is_exact() {
        let c = compare(&rect(0.0, 0.0, 100.0, 100.0), &BezPath::new());
        assert!((c.reference_area - 10_000.0).abs() < 1e-6, "{c:?}");
        // Counter-clockwise (y-up) winds +1.
        let c = compare(&BezPath::new(), &rect(0.0, 0.0, 100.0, 100.0));
        assert!((c.positive_area - 10_000.0).abs() < 1e-6, "{c:?}");
        assert_eq!(c.negative_area, 0.0);
        assert_eq!(structure(&rect(0.0, 0.0, 100.0, 100.0)).segments, 3);
    }

    #[test]
    fn circle_area_within_tolerance() {
        let circle = Circle::new((0.0, 0.0), 300.0).to_path(1e-6);
        let c = compare(&BezPath::new(), &circle);
        let exact = std::f64::consts::PI * 300.0 * 300.0;
        assert!((c.output_area - exact).abs() / exact < 1e-5, "{c:?}");
        assert_eq!(c.overlap_area, 0.0);
        assert_eq!(c.mixed_winding_area(), 0.0);
    }

    #[test]
    fn overlap_and_union_are_measured() {
        let mut two = rect(0.0, 0.0, 100.0, 100.0);
        two.extend(rect(50.0, 0.0, 150.0, 100.0));
        let union = rect(0.0, 0.0, 150.0, 100.0);
        let c = compare(&two, &union);
        assert!((c.reference_area - 15_000.0).abs() < 1e-6, "{c:?}");
        assert!(c.xor_area < 1e-6, "{c:?}");
        let profile = compare(&BezPath::new(), &two);
        assert!((profile.overlap_area - 5_000.0).abs() < 1e-6, "{profile:?}");
    }

    #[test]
    fn opposite_directions_are_mixed_winding() {
        let mut two = rect(0.0, 0.0, 100.0, 100.0);
        two.extend(rect(200.0, 0.0, 300.0, 100.0).reverse_subpaths());
        let c = compare(&BezPath::new(), &two);
        assert!((c.mixed_winding_area() - 10_000.0).abs() < 1e-6, "{c:?}");
        // kurbo's Rect path is counter-clockwise in y-up, the PostScript direction.
        assert!(rect(0.0, 0.0, 1.0, 1.0).area() > 0.0);
        assert!(c.positive_area > 0.0);
    }

    #[test]
    fn structure_flags_open_and_non_finite() {
        let mut open = BezPath::new();
        open.move_to((0.0, 0.0));
        open.line_to((10.0, 0.0));
        open.line_to((f64::NAN, 5.0));
        let s = structure(&open);
        assert_eq!((s.contours, s.open_contours, s.non_finite), (1, 1, true));
        let mut two = rect(0.0, 0.0, 10.0, 10.0);
        two.extend(rect(20.0, 0.0, 20.5, 1.0));
        let s = structure(&two);
        assert_eq!(
            (s.contours, s.open_contours, s.non_finite, s.tiny_contours),
            (2, 0, false, 1)
        );
    }
}
