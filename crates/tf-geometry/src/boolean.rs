//! The `BooleanEngine` port: boolean operations on outlines (implementation plan §5.2,
//! ADR-0008, M1 deviation 6).
//!
//! Every operation reads its inputs under the non-zero fill rule, the rule of TrueType and
//! CFF outlines, and returns closed contours that do not overlap, wound for that rule
//! (outer contours counter-clockwise in a y-up system, counters clockwise). The engine
//! itself lives in an adapter crate (`tf-boolean`), so this kernel stays free of it and
//! keeps compiling to WASM.

use std::fmt;

use kurbo::BezPath;

/// Why an engine refused its input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BooleanError {
    /// A coordinate is NaN or infinite.
    NonFinite,
    /// A contour does not end where it starts.
    OpenContour,
}

impl fmt::Display for BooleanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NonFinite => "a coordinate is NaN or infinite",
            Self::OpenContour => "a contour is not closed",
        })
    }
}

impl std::error::Error for BooleanError {}

/// Boolean operations on outlines under the non-zero fill rule.
///
/// `Send + Sync`: glyphs are independent, so export runs them in parallel (ADR-0008).
pub trait BooleanEngine: Send + Sync {
    /// The outline with its overlaps removed: the union of its own contours.
    fn remove_overlap(&self, path: &BezPath) -> Result<BezPath, BooleanError>;
    /// The area inside `a` or `b`.
    fn union(&self, a: &BezPath, b: &BezPath) -> Result<BezPath, BooleanError>;
    /// The area inside both `a` and `b`.
    fn intersection(&self, a: &BezPath, b: &BezPath) -> Result<BezPath, BooleanError>;
    /// The area inside `a` and outside `b`.
    fn difference(&self, a: &BezPath, b: &BezPath) -> Result<BezPath, BooleanError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_say_what_is_wrong_with_the_input() {
        assert_eq!(
            BooleanError::NonFinite.to_string(),
            "a coordinate is NaN or infinite"
        );
        assert_eq!(
            BooleanError::OpenContour.to_string(),
            "a contour is not closed"
        );
    }
}
