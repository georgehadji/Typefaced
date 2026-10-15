//! Packed outlines (implementation plan §7.2, ADR-0002) and the binary form of
//! `glyph.movePoints`. Both codecs are little-endian, validate every length, flag,
//! index and coordinate, and never panic on malformed bytes.
//!
//! Packed outline bytes: `u32` point count `n`, `u32` contour count `c`, `2n` `f64`
//! coordinates, `n` `u8` flags, zero padding to a multiple of 4, `c` `u32` contour ends.
//!
//! `glyph.movePoints` bytes: `u32` glyph, `u32` layer, `u32` count `k`, `k` `u32` point
//! indices, `2k` `f64` coordinates.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use specta::Type;
use tf_geometry::flags;

use crate::command::{GlyphId, GlyphMovePoints, LayerId};
use crate::error::{CommandError, ErrorCode};

/// The flag bits a point may set: the point type and the smooth bit.
const VALID_FLAG_BITS: u8 = flags::TYPE_MASK | flags::SMOOTH;

/// An outline as three parallel arrays, the layout the UI holds in typed arrays.
/// Contours are closed; `contour_ends` holds the index of each contour's last point.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackedOutline {
    /// Interleaved `[x0, y0, x1, y1, …]` in font units; finite.
    pub coords: Vec<f64>,
    /// One byte per point: the point type (`0` off-curve, `1` line, `2` curve,
    /// `3` qcurve) plus `4` for a smooth point.
    pub flags: Vec<u8>,
    /// The index of the last point of each contour, strictly increasing; the last one is
    /// the last point, so the contours cover every point.
    pub contour_ends: Vec<u32>,
}

/// Why a packed payload is invalid.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PackedError {
    /// The input is shorter than its header, or the header asks for more bytes than
    /// memory can address.
    #[error("truncated input of {0} bytes")]
    Truncated(usize),
    /// The byte length does not match the counts in the header.
    #[error("expected {expected} bytes, got {actual}")]
    Length {
        /// Bytes the header asks for.
        expected: usize,
        /// Bytes received.
        actual: usize,
    },
    /// The coordinate array is not twice the point count.
    #[error("{coords} coordinates for {points} points")]
    CoordCount {
        /// Number of coordinates.
        coords: usize,
        /// Number of points.
        points: usize,
    },
    /// More points than a `u32` count can hold.
    #[error("too many points: {0}")]
    TooManyPoints(usize),
    /// A coordinate is NaN or infinite.
    #[error("coordinate {0} is not finite")]
    NonFinite(usize),
    /// A flag sets unknown bits.
    #[error("point {index} has invalid flags {flag:#04x}")]
    BadFlag {
        /// Point index.
        index: usize,
        /// The flag byte.
        flag: u8,
    },
    /// A padding byte is not zero.
    #[error("non-zero padding")]
    Padding,
    /// Contour ends are not strictly increasing, are out of range, or leave points out.
    #[error("contour end {index} is invalid")]
    ContourEnd {
        /// Index into `contour_ends` (the count when the last end is missing).
        index: usize,
    },
    /// Point indices are not strictly increasing.
    #[error("point index {index} is not above the previous one")]
    IndexOrder {
        /// Position in the index list.
        index: usize,
    },
}

impl From<PackedError> for CommandError {
    fn from(error: PackedError) -> Self {
        CommandError::new(ErrorCode::InvalidOutline, error.to_string())
    }
}

impl PackedOutline {
    /// Number of points.
    pub fn point_count(&self) -> usize {
        self.flags.len()
    }

    /// Checks lengths, finite coordinates, flag bits and contour ends.
    pub fn validate(&self) -> Result<(), PackedError> {
        let n = self.point_count();
        u32::try_from(n).map_err(|_| PackedError::TooManyPoints(n))?;
        check_coords(&self.coords, n)?;
        if let Some(index) = self.flags.iter().position(|f| f & !VALID_FLAG_BITS != 0) {
            return Err(PackedError::BadFlag {
                index,
                flag: self.flags[index],
            });
        }
        check_contour_ends(&self.contour_ends, n)
    }

    /// The binary form, after [`validate`](Self::validate).
    pub fn encode(&self) -> Result<Vec<u8>, PackedError> {
        self.validate()?;
        let n = self.point_count();
        let mut out = Vec::with_capacity(outline_size(n, self.contour_ends.len()).unwrap_or(0));
        push_u32(&mut out, n);
        push_u32(&mut out, self.contour_ends.len());
        self.coords
            .iter()
            .for_each(|c| out.extend_from_slice(&c.to_le_bytes()));
        out.extend_from_slice(&self.flags);
        out.resize(out.len() + padding(n), 0);
        self.contour_ends
            .iter()
            .for_each(|e| out.extend_from_slice(&e.to_le_bytes()));
        Ok(out)
    }

    /// Parses and validates the binary form.
    pub fn decode(bytes: &[u8]) -> Result<Self, PackedError> {
        let mut r = Reader::new(bytes);
        let n = r.u32()? as usize;
        let c = r.u32()? as usize;
        r.expect_total(outline_size(n, c))?;
        let coords = r.f64s(2 * n)?;
        let flags = r.take(n)?.to_vec();
        if r.take(padding(n))?.iter().any(|&b| b != 0) {
            return Err(PackedError::Padding);
        }
        let contour_ends = r.u32s(c)?;
        let outline = Self {
            coords,
            flags,
            contour_ends,
        };
        outline.validate()?;
        Ok(outline)
    }
}

impl GlyphMovePoints {
    /// Checks that there is one coordinate pair per index, that the indices strictly
    /// increase and that every coordinate is finite. The engine checks the indices
    /// against the glyph.
    pub fn validate(&self) -> Result<(), PackedError> {
        let k = self.indices.len();
        u32::try_from(k).map_err(|_| PackedError::TooManyPoints(k))?;
        check_coords(&self.coords, k)?;
        match self.indices.windows(2).position(|w| w[0] >= w[1]) {
            Some(i) => Err(PackedError::IndexOrder { index: i + 1 }),
            None => Ok(()),
        }
    }

    /// The binary form, after [`validate`](Self::validate).
    pub fn encode(&self) -> Result<Vec<u8>, PackedError> {
        self.validate()?;
        let k = self.indices.len();
        let mut out = Vec::with_capacity(move_points_size(k).unwrap_or(0));
        out.extend_from_slice(&self.id.0.to_le_bytes());
        out.extend_from_slice(&self.layer.0.to_le_bytes());
        push_u32(&mut out, k);
        self.indices
            .iter()
            .for_each(|i| out.extend_from_slice(&i.to_le_bytes()));
        self.coords
            .iter()
            .for_each(|c| out.extend_from_slice(&c.to_le_bytes()));
        Ok(out)
    }

    /// Parses and validates the binary form.
    pub fn decode(bytes: &[u8]) -> Result<Self, PackedError> {
        let mut r = Reader::new(bytes);
        let id = GlyphId(r.u32()?);
        let layer = LayerId(r.u32()?);
        let k = r.u32()? as usize;
        r.expect_total(move_points_size(k))?;
        let command = Self {
            id,
            layer,
            indices: r.u32s(k)?,
            coords: r.f64s(2 * k)?,
        };
        command.validate()?;
        Ok(command)
    }
}

fn check_coords(coords: &[f64], points: usize) -> Result<(), PackedError> {
    if points.checked_mul(2) != Some(coords.len()) {
        return Err(PackedError::CoordCount {
            coords: coords.len(),
            points,
        });
    }
    match coords.iter().position(|c| !c.is_finite()) {
        Some(i) => Err(PackedError::NonFinite(i)),
        None => Ok(()),
    }
}

fn check_contour_ends(ends: &[u32], points: usize) -> Result<(), PackedError> {
    let mut next_start = 0usize;
    for (index, &end) in ends.iter().enumerate() {
        let end = end as usize;
        if end < next_start || end >= points {
            return Err(PackedError::ContourEnd { index });
        }
        next_start = end + 1;
    }
    if next_start == points {
        Ok(())
    } else {
        Err(PackedError::ContourEnd { index: ends.len() })
    }
}

/// Zero bytes after `n` flags, so the contour ends start at a multiple of 4.
fn padding(n: usize) -> usize {
    (4 - n % 4) % 4
}

fn outline_size(n: usize, c: usize) -> Option<usize> {
    let coords = n.checked_mul(16)?;
    let ends = c.checked_mul(4)?;
    8usize
        .checked_add(coords)?
        .checked_add(n)?
        .checked_add(padding(n))?
        .checked_add(ends)
}

fn move_points_size(k: usize) -> Option<usize> {
    12usize.checked_add(k.checked_mul(4 + 16)?)
}

/// Appends a count that [`PackedOutline::validate`] or
/// [`GlyphMovePoints::validate`] has already bounded to `u32`.
fn push_u32(out: &mut Vec<u8>, value: usize) {
    out.extend_from_slice(&(value as u32).to_le_bytes());
}

/// A cursor over the input bytes; every read checks the remaining length.
struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    /// Fails unless the whole input is exactly `total` bytes, before anything is
    /// allocated for the counts in the header.
    fn expect_total(&self, total: Option<usize>) -> Result<(), PackedError> {
        match total {
            Some(expected) if expected == self.bytes.len() => Ok(()),
            Some(expected) => Err(PackedError::Length {
                expected,
                actual: self.bytes.len(),
            }),
            None => Err(PackedError::Truncated(self.bytes.len())),
        }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], PackedError> {
        let slice = self
            .pos
            .checked_add(len)
            .and_then(|end| self.bytes.get(self.pos..end))
            .ok_or(PackedError::Truncated(self.bytes.len()))?;
        self.pos += len;
        Ok(slice)
    }

    fn u32(&mut self) -> Result<u32, PackedError> {
        // `u32s(1)` returns exactly one word or an error.
        Ok(self.u32s(1)?[0])
    }

    fn u32s(&mut self, count: usize) -> Result<Vec<u32>, PackedError> {
        let (words, _) = self.take(count.saturating_mul(4))?.as_chunks::<4>();
        Ok(words.iter().map(|&w| u32::from_le_bytes(w)).collect())
    }

    fn f64s(&mut self, count: usize) -> Result<Vec<f64>, PackedError> {
        let (words, _) = self.take(count.saturating_mul(8))?.as_chunks::<8>();
        Ok(words.iter().map(|&w| f64::from_le_bytes(w)).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn triangle() -> PackedOutline {
        PackedOutline {
            coords: vec![0.0, 0.0, 100.0, 0.0, 50.0, 80.0],
            flags: vec![flags::LINE, flags::LINE | flags::SMOOTH, flags::LINE],
            contour_ends: vec![2],
        }
    }

    fn move_points() -> GlyphMovePoints {
        GlyphMovePoints {
            id: GlyphId(7),
            layer: LayerId(1),
            indices: vec![0, 2],
            coords: vec![1.5, -2.0, 300.0, 400.25],
        }
    }

    #[test]
    fn a_triangle_has_the_documented_byte_layout() {
        let bytes = triangle().encode().unwrap();

        assert_eq!(bytes.len(), 8 + 48 + 3 + 1 + 4);
        assert_eq!(&bytes[..8], &[3, 0, 0, 0, 1, 0, 0, 0]);
        assert_eq!(&bytes[8 + 16..8 + 24], &100.0f64.to_le_bytes());
        assert_eq!(&bytes[56..60], &[1, 5, 1, 0]);
        assert_eq!(&bytes[60..], &[2, 0, 0, 0]);
        assert_eq!(PackedOutline::decode(&bytes).unwrap(), triangle());
    }

    #[test]
    fn an_empty_outline_round_trips() {
        let empty = PackedOutline::default();

        let bytes = empty.encode().unwrap();

        assert_eq!(bytes, vec![0; 8]);
        assert_eq!(PackedOutline::decode(&bytes).unwrap(), empty);
    }

    #[test]
    fn truncated_and_overlong_bytes_are_rejected() {
        let bytes = triangle().encode().unwrap();

        for len in 0..8 {
            assert_eq!(
                PackedOutline::decode(&bytes[..len]),
                Err(PackedError::Truncated(len))
            );
        }
        for len in 8..bytes.len() {
            assert_eq!(
                PackedOutline::decode(&bytes[..len]),
                Err(PackedError::Length {
                    expected: bytes.len(),
                    actual: len
                })
            );
        }
        let mut long = bytes.clone();
        long.push(0);
        assert_eq!(
            PackedOutline::decode(&long),
            Err(PackedError::Length {
                expected: bytes.len(),
                actual: bytes.len() + 1
            })
        );
    }

    #[test]
    fn a_header_with_huge_counts_fails_without_allocating() {
        let bytes = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];

        assert!(matches!(
            PackedOutline::decode(&bytes),
            Err(PackedError::Length { .. })
        ));
        let mut move_bytes = vec![0u8; 8];
        move_bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            GlyphMovePoints::decode(&move_bytes),
            Err(PackedError::Length { .. })
        ));
    }

    #[test]
    fn a_bad_flag_is_rejected() {
        let mut bytes = triangle().encode().unwrap();
        bytes[57] = 0b1000;

        assert_eq!(
            PackedOutline::decode(&bytes),
            Err(PackedError::BadFlag {
                index: 1,
                flag: 0b1000
            })
        );
    }

    #[test]
    fn non_zero_padding_is_rejected() {
        let mut bytes = triangle().encode().unwrap();
        bytes[59] = 1;

        assert_eq!(PackedOutline::decode(&bytes), Err(PackedError::Padding));
    }

    #[test]
    fn unordered_out_of_range_or_partial_contour_ends_are_rejected() {
        let with_ends = |ends: Vec<u32>| PackedOutline {
            contour_ends: ends,
            ..triangle()
        };

        let unordered = with_ends(vec![1, 1, 2]);
        let out_of_range = with_ends(vec![3]);
        let partial = with_ends(vec![1]);
        let none = with_ends(vec![]);

        assert_eq!(
            unordered.validate(),
            Err(PackedError::ContourEnd { index: 1 })
        );
        assert_eq!(
            out_of_range.validate(),
            Err(PackedError::ContourEnd { index: 0 })
        );
        assert_eq!(
            partial.validate(),
            Err(PackedError::ContourEnd { index: 1 })
        );
        assert_eq!(none.validate(), Err(PackedError::ContourEnd { index: 0 }));
        let mut bytes = triangle().encode().unwrap();
        bytes[60] = 5;
        assert_eq!(
            PackedOutline::decode(&bytes),
            Err(PackedError::ContourEnd { index: 0 })
        );
    }

    #[test]
    fn mismatched_coordinates_and_non_finite_values_are_rejected() {
        let short = PackedOutline {
            coords: vec![0.0; 5],
            ..triangle()
        };
        let nan = PackedOutline {
            coords: vec![0.0, 0.0, f64::NAN, 0.0, 1.0, 1.0],
            ..triangle()
        };

        assert_eq!(
            short.encode(),
            Err(PackedError::CoordCount {
                coords: 5,
                points: 3
            })
        );
        assert_eq!(nan.validate(), Err(PackedError::NonFinite(2)));
        let mut bytes = triangle().encode().unwrap();
        bytes[8..16].copy_from_slice(&f64::INFINITY.to_le_bytes());
        assert_eq!(
            PackedOutline::decode(&bytes),
            Err(PackedError::NonFinite(0))
        );
    }

    #[test]
    fn move_points_has_the_documented_byte_layout() {
        let bytes = move_points().encode().unwrap();

        assert_eq!(bytes.len(), 12 + 8 + 32);
        assert_eq!(&bytes[..12], &[7, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0]);
        assert_eq!(&bytes[12..20], &[0, 0, 0, 0, 2, 0, 0, 0]);
        assert_eq!(&bytes[44..], &400.25f64.to_le_bytes());
        assert_eq!(GlyphMovePoints::decode(&bytes).unwrap(), move_points());
    }

    #[test]
    fn move_points_rejects_bad_order_counts_and_lengths() {
        let unordered = GlyphMovePoints {
            indices: vec![2, 2],
            ..move_points()
        };
        let short = GlyphMovePoints {
            coords: vec![1.0],
            ..move_points()
        };

        assert_eq!(
            unordered.encode(),
            Err(PackedError::IndexOrder { index: 1 })
        );
        assert_eq!(
            short.validate(),
            Err(PackedError::CoordCount {
                coords: 1,
                points: 2
            })
        );
        let bytes = move_points().encode().unwrap();
        assert!(matches!(
            GlyphMovePoints::decode(&bytes[..bytes.len() - 1]),
            Err(PackedError::Length { .. })
        ));
        let mut swapped = bytes.clone();
        swapped[12..20].copy_from_slice(&[2, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(
            GlyphMovePoints::decode(&swapped),
            Err(PackedError::IndexOrder { index: 1 })
        );
    }

    #[test]
    fn a_packed_error_becomes_an_invalid_outline_command_error() {
        let error: CommandError = PackedError::Padding.into();

        assert_eq!(error.code, ErrorCode::InvalidOutline);
        assert_eq!(error.message, "non-zero padding");
    }

    /// Valid outlines: finite coordinates, valid flags, contour ends that cover all points.
    fn outline() -> impl Strategy<Value = PackedOutline> {
        prop::collection::vec(
            (
                -1e6..1e6f64,
                -1e6..1e6f64,
                0u8..=VALID_FLAG_BITS,
                any::<bool>(),
            ),
            0..64,
        )
        .prop_map(|points| {
            let n = points.len();
            let mut ends: Vec<u32> = points
                .iter()
                .enumerate()
                .filter(|(i, p)| p.3 && i + 1 < n)
                .map(|(i, _)| i as u32)
                .collect();
            if n > 0 {
                ends.push(n as u32 - 1);
            }
            PackedOutline {
                coords: points.iter().flat_map(|p| [p.0, p.1]).collect(),
                flags: points.iter().map(|p| p.2).collect(),
                contour_ends: ends,
            }
        })
    }

    proptest! {
        #[test]
        fn packed_outlines_round_trip(outline in outline()) {
            let bytes = outline.encode().unwrap();

            prop_assert_eq!(bytes.len() % 4, 0);
            prop_assert_eq!(PackedOutline::decode(&bytes).unwrap(), outline);
        }

        #[test]
        fn move_points_round_trip(
            id in any::<u32>(),
            layer in any::<u32>(),
            points in prop::collection::btree_map(any::<u32>(), (-1e6..1e6f64, -1e6..1e6f64), 0..64),
        ) {
            let command = GlyphMovePoints {
                id: GlyphId(id),
                layer: LayerId(layer),
                indices: points.keys().copied().collect(),
                coords: points.values().flat_map(|&(x, y)| [x, y]).collect(),
            };

            let bytes = command.encode().unwrap();

            prop_assert_eq!(GlyphMovePoints::decode(&bytes).unwrap(), command);
        }

        #[test]
        fn arbitrary_bytes_never_panic(bytes in prop::collection::vec(any::<u8>(), 0..256)) {
            let _ = PackedOutline::decode(&bytes);
            let _ = GlyphMovePoints::decode(&bytes);
        }

        #[test]
        fn a_flipped_byte_never_panics_and_valid_results_re_encode(
            outline in outline(),
            at in any::<prop::sample::Index>(),
            value in any::<u8>(),
        ) {
            let mut bytes = outline.encode().unwrap();
            let i = at.index(bytes.len());
            bytes[i] = value;

            if let Ok(decoded) = PackedOutline::decode(&bytes) {
                prop_assert_eq!(decoded.encode().unwrap(), bytes);
            }
        }
    }
}
