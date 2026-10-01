//! WebAssembly facade over `tf-geometry` (implementation plan §5.2, ADR-013).
//!
//! Calls are coarse-grained and take typed arrays, so one call per frame does the work.
//! Build: `wasm-pack build crates/tf-wasm --target web --out-dir ../../packages/geometry-wasm/pkg`.

use tf_geometry::Hit;
use wasm_bindgen::prelude::wasm_bindgen;

/// Hit-tests a packed outline. Returns an empty array when nothing is within `radius`,
/// `[0, index, distance]` for a point, or `[1, contour, start, end, t, distance]` for a
/// segment. A plain `Float64Array` leaves no wasm-bindgen object for JavaScript to free.
#[wasm_bindgen(js_name = hitTest)]
pub fn hit_test(
    coords: &[f64],
    flags: &[u8],
    contour_ends: &[u32],
    x: f64,
    y: f64,
    radius: f64,
) -> Vec<f64> {
    encode_hit(tf_geometry::hit_test(
        coords,
        flags,
        contour_ends,
        x,
        y,
        radius,
    ))
}

/// Returns a copy of `coords` with the selected points moved by `(dx, dy)`.
#[wasm_bindgen(js_name = translatePoints)]
pub fn translate_points(coords: &[f64], selection: &[u32], dx: f64, dy: f64) -> Vec<f64> {
    tf_geometry::translate_points(coords, selection, dx, dy)
}

/// Indices travel as `f64`, which is exact for every index below 2^53.
#[allow(
    clippy::cast_precision_loss,
    reason = "outline indices are far below 2^53"
)]
fn encode_hit(hit: Option<Hit>) -> Vec<f64> {
    match hit {
        None => Vec::new(),
        Some(Hit::Point { index, distance }) => vec![0.0, index as f64, distance],
        Some(Hit::Segment {
            contour,
            start,
            end,
            t,
            distance,
        }) => vec![1.0, contour as f64, start as f64, end as f64, t, distance],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hits_encode_as_flat_arrays() {
        assert!(encode_hit(None).is_empty());
        assert_eq!(
            encode_hit(Some(Hit::Point {
                index: 7,
                distance: 1.5
            })),
            [0.0, 7.0, 1.5]
        );
        assert_eq!(
            encode_hit(Some(Hit::Segment {
                contour: 1,
                start: 4,
                end: 6,
                t: 0.25,
                distance: 2.0
            })),
            [1.0, 1.0, 4.0, 6.0, 0.25, 2.0]
        );
    }
}
