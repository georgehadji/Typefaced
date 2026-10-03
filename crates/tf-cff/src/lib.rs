//! Minimal CFF (version 1) writer: cubic outlines in, a `CFF ` table out (ADR-0007).
//! Pure: no I/O. Specs: Adobe TN #5176 (CFF) and TN #5177 (Type 2 charstrings).
//!
//! One font, no subroutines, no hints, charset format 0, standard encoding. Points
//! are rounded to integers with OpenType rounding.

mod encode;
mod outline;

use std::collections::{BTreeMap, HashMap, HashSet};

use kurbo::BezPath;
use read_fonts::ps::string::Sid;

use encode::{dict_int, dict_int32, dict_real, index};

// DICT operators (TN #5176 Table 9 and Table 23).
const FONT_BBOX: &[u8] = &[5];
const FONT_MATRIX: &[u8] = &[12, 7];
const CHARSET: &[u8] = &[15];
const CHAR_STRINGS: &[u8] = &[17];
const PRIVATE: &[u8] = &[18];
const DEFAULT_WIDTH_X: &[u8] = &[20];
const NOMINAL_WIDTH_X: &[u8] = &[21];

/// The largest string ID (TN #5176 Table 2: SIDs are 0–64999).
const MAX_SID: u16 = 64_999;

/// Why a CFF table could not be built.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CffError {
    /// The first glyph is not `.notdef`, or there are no glyphs.
    #[error("the first glyph must be .notdef")]
    NotdefNotFirst,
    #[error("glyph name {0:?} is used twice")]
    DuplicateGlyphName(String),
    /// Glyph names must be printable ASCII (33–126) and not empty.
    #[error("glyph name {0:?} is empty or has characters outside printable ASCII")]
    InvalidGlyphName(String),
    /// TN #5176 §7: 1–63 printable ASCII characters, none of `[](){}<>/%`.
    #[error("{0:?} is not a valid PostScript font name")]
    InvalidFontName(String),
    /// The OpenType range is 16–16384.
    #[error("units per em must be 16 to 16384, got {0}")]
    InvalidUnitsPerEm(u16),
    #[error("{0} glyphs: CFF holds at most 65535")]
    TooManyGlyphs(usize),
    /// Custom glyph names get string IDs 391–64999 (TN #5176 Table 2).
    #[error("{0} custom glyph names: their string IDs would exceed 64999")]
    TooManyStrings(usize),
    #[error("the table is too large for CFF offsets")]
    IndexTooLarge,
    #[error("glyph {name:?}: {error}")]
    Glyph { name: String, error: OutlineError },
}

/// Why a glyph outline cannot be stored in CFF.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum OutlineError {
    #[error("quadratic segment: CFF outlines are cubic")]
    Quadratic,
    #[error("a subpath does not start with a move")]
    MissingMove,
    #[error("a coordinate is not finite")]
    NonFinite,
    /// A charstring number (coordinate delta or width delta) beyond 16 bits.
    #[error("{0} does not fit a 16-bit charstring number")]
    OutOfRange(i64),
    /// TN #5177 Appendix B: a charstring holds at most 65535 bytes.
    #[error("the charstring takes {0} bytes; CFF allows 65535")]
    TooLong(usize),
}

/// Integer glyph bounds, as the font states them (`FontBBox`, and for the caller the
/// `head`, `hmtx` and `hhea` values).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    pub x_min: i32,
    pub y_min: i32,
    pub x_max: i32,
    pub y_max: i32,
}

impl Bounds {
    /// The smallest bounds holding both.
    pub fn union(self, other: Self) -> Self {
        Self {
            x_min: self.x_min.min(other.x_min),
            y_min: self.y_min.min(other.y_min),
            x_max: self.x_max.max(other.x_max),
            y_max: self.y_max.max(other.y_max),
        }
    }
}

/// The bounds of `path` as [`CffBuilder`] encodes it (points rounded with
/// `floor(v + 0.5)`, subpaths without segments left out): the exact curve extrema,
/// rounded outwards, as fontTools computes them from a `CFF ` table. `None` when
/// nothing is drawn.
pub fn bounds(path: &BezPath) -> Result<Option<Bounds>, OutlineError> {
    outline::subpaths(path).map(|subpaths| outline::bounds(&subpaths))
}

/// Builds a CFF table for one font from glyphs in glyph order.
///
/// ```text
/// CffBuilder::new("MyFont-Regular", 1000)
///     .glyph(".notdef", 500, &notdef)
///     .glyph("A", 600, &a)
///     .build()?
/// ```
#[derive(Debug, Clone)]
pub struct CffBuilder {
    font_name: String,
    units_per_em: u16,
    glyphs: Vec<Glyph>,
}

#[derive(Debug, Clone)]
struct Glyph {
    name: String,
    advance: u16,
    path: BezPath,
}

impl CffBuilder {
    /// `font_name` goes in the Name INDEX: the PostScript name (name ID 6).
    pub fn new(font_name: impl Into<String>, units_per_em: u16) -> Self {
        Self {
            font_name: font_name.into(),
            units_per_em,
            glyphs: Vec::new(),
        }
    }

    /// Adds the next glyph. The first must be `.notdef`. Every subpath of `path` is
    /// closed, as CFF has no open contours. Errors are reported by [`Self::build`].
    #[must_use]
    pub fn glyph(mut self, name: impl Into<String>, advance: u16, path: &BezPath) -> Self {
        self.glyphs.push(Glyph {
            name: name.into(),
            advance,
            path: path.clone(),
        });
        self
    }

    /// The `CFF ` table bytes.
    pub fn build(self) -> Result<Vec<u8>, CffError> {
        check_font_name(&self.font_name)?;
        if !(16..=16384).contains(&self.units_per_em) {
            return Err(CffError::InvalidUnitsPerEm(self.units_per_em));
        }
        if self.glyphs.len() > usize::from(u16::MAX) {
            return Err(CffError::TooManyGlyphs(self.glyphs.len()));
        }
        if self.glyphs.first().map(|g| g.name.as_str()) != Some(".notdef") {
            return Err(CffError::NotdefNotFirst);
        }
        let strings = Strings::assign(&self.glyphs)?;
        let width = most_common_advance(&self.glyphs);
        let mut charstrings = Vec::with_capacity(self.glyphs.len());
        let mut bbox: Option<Bounds> = None;
        for glyph in &self.glyphs {
            let error = |error| CffError::Glyph {
                name: glyph.name.clone(),
                error,
            };
            let subpaths = outline::subpaths(&glyph.path).map_err(error)?;
            if let Some(bounds) = outline::bounds(&subpaths) {
                bbox = Some(bbox.map_or(bounds, |b| b.union(bounds)));
            }
            let delta =
                (glyph.advance != width).then(|| i64::from(glyph.advance) - i64::from(width));
            charstrings.push(outline::charstring(&subpaths, delta).map_err(error)?);
        }
        let mut private = Vec::new();
        dict_int(&mut private, i32::from(width));
        private.extend(DEFAULT_WIDTH_X);
        dict_int(&mut private, i32::from(width));
        private.extend(NOMINAL_WIDTH_X);
        Layout {
            font_name: &self.font_name,
            units_per_em: self.units_per_em,
            bbox,
            strings: &strings,
            charstrings: &charstrings,
            private: &private,
        }
        .write()
    }
}

/// TN #5176 §7 restrictions on the FontName.
fn check_font_name(name: &str) -> Result<(), CffError> {
    let valid = (1..=63).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_graphic() && !b"[](){}<>/%".contains(&b));
    if valid {
        Ok(())
    } else {
        Err(CffError::InvalidFontName(name.to_owned()))
    }
}

/// The advance most glyphs share (the smallest on a tie). It becomes both
/// `defaultWidthX`, so those glyphs store no width, and `nominalWidthX`.
fn most_common_advance(glyphs: &[Glyph]) -> u16 {
    let mut counts = BTreeMap::new();
    for glyph in glyphs {
        *counts.entry(glyph.advance).or_insert(0_usize) += 1;
    }
    counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
        .map_or(0, |(advance, _)| advance)
}

/// Glyph-name string IDs: standard strings keep their SID (0–390), other names go in
/// the String INDEX from SID 391 on.
struct Strings {
    /// The SID of every glyph, in glyph order.
    sids: Vec<u16>,
    custom: Vec<Vec<u8>>,
}

impl Strings {
    fn assign(glyphs: &[Glyph]) -> Result<Self, CffError> {
        let standard: HashMap<&[u8], u16> = (0..=u16::MAX)
            .map_while(|sid| Sid::new(sid).resolve_standard().ok().map(|s| (s, sid)))
            .collect();
        let first_custom = standard.len();
        let mut seen = HashSet::new();
        let mut custom = Vec::new();
        let mut sids = Vec::with_capacity(glyphs.len());
        for glyph in glyphs {
            let name = glyph.name.as_bytes();
            if name.is_empty() || !name.iter().all(u8::is_ascii_graphic) {
                return Err(CffError::InvalidGlyphName(glyph.name.clone()));
            }
            if !seen.insert(name) {
                return Err(CffError::DuplicateGlyphName(glyph.name.clone()));
            }
            let sid = match standard.get(name) {
                Some(&sid) => usize::from(sid),
                None => {
                    custom.push(name.to_vec());
                    first_custom + custom.len() - 1
                }
            };
            let sid = u16::try_from(sid)
                .ok()
                .filter(|&sid| sid <= MAX_SID)
                .ok_or(CffError::TooManyStrings(custom.len()))?;
            sids.push(sid);
        }
        Ok(Self { sids, custom })
    }
}

/// The table layout, in order: Header, Name INDEX, Top DICT INDEX, String INDEX,
/// Global Subr INDEX (empty), charset, CharStrings INDEX, Private DICT.
struct Layout<'a> {
    font_name: &'a str,
    units_per_em: u16,
    bbox: Option<Bounds>,
    strings: &'a Strings,
    charstrings: &'a [Vec<u8>],
    private: &'a [u8],
}

struct Offsets {
    charset: i32,
    charstrings: i32,
    private: i32,
}

impl Layout<'_> {
    fn write(&self) -> Result<Vec<u8>, CffError> {
        // Header: major 1, minor 0, header size 4, absolute offset size 4.
        let mut out = vec![1, 0, 4, 4];
        index(&mut out, &[self.font_name.as_bytes().to_vec()])?;

        let mut strings = Vec::new();
        index(&mut strings, &self.strings.custom)?;
        let mut charset = vec![0]; // format 0: one SID per glyph after .notdef
        for sid in &self.strings.sids[1..] {
            charset.extend(sid.to_be_bytes());
        }
        let mut charstrings = Vec::new();
        index(&mut charstrings, self.charstrings)?;

        // Offsets use the fixed 5-byte form, so the Top DICT length does not depend on
        // them: measure it with zero offsets, then write it with the real ones.
        let unplaced = Offsets {
            charset: 0,
            charstrings: 0,
            private: 0,
        };
        let mut top_index = Vec::new();
        index(&mut top_index, &[self.top_dict(&unplaced)?])?;
        let empty_global_subrs = [0, 0];
        let charset_at = out.len() + top_index.len() + strings.len() + empty_global_subrs.len();
        let offset = |at: usize| i32::try_from(at).map_err(|_| CffError::IndexTooLarge);
        let placed = Offsets {
            charset: offset(charset_at)?,
            charstrings: offset(charset_at + charset.len())?,
            private: offset(charset_at + charset.len() + charstrings.len())?,
        };

        index(&mut out, &[self.top_dict(&placed)?])?;
        out.extend(strings);
        out.extend(empty_global_subrs);
        out.extend(charset);
        out.extend(charstrings);
        out.extend(self.private);
        Ok(out)
    }

    fn top_dict(&self, offsets: &Offsets) -> Result<Vec<u8>, CffError> {
        let mut dict = Vec::new();
        if let Some(b) = self.bbox {
            for v in [b.x_min, b.y_min, b.x_max, b.y_max] {
                dict_int(&mut dict, v);
            }
            dict.extend(FONT_BBOX);
        }
        if self.units_per_em != 1000 {
            let scale = 1.0 / f64::from(self.units_per_em);
            dict_real(&mut dict, scale);
            dict_int(&mut dict, 0);
            dict_int(&mut dict, 0);
            dict_real(&mut dict, scale);
            dict_int(&mut dict, 0);
            dict_int(&mut dict, 0);
            dict.extend(FONT_MATRIX);
        }
        dict_int32(&mut dict, offsets.charset);
        dict.extend(CHARSET);
        dict_int32(&mut dict, offsets.charstrings);
        dict.extend(CHAR_STRINGS);
        let private_len = i32::try_from(self.private.len()).map_err(|_| CffError::IndexTooLarge)?;
        dict_int(&mut dict, private_len);
        dict_int32(&mut dict, offsets.private);
        dict.extend(PRIVATE);
        Ok(dict)
    }
}
