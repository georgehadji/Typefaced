//! TTF → OTF transplant (ADR-0007; implementation plan §5.13, stage 6): keeps the
//! tables fontc built that do not depend on outlines, and replaces the TrueType
//! outlines with a `CFF ` table built from the cubic source outlines.

use kurbo::BezPath;
use tf_cff::{CffBuilder, CffError};
use write_fonts::FontBuilder;
use write_fonts::from_obj::ToOwnedTable;
use write_fonts::read::tables::hmtx::Hmtx as ReadHmtx;
use write_fonts::read::{FontRef, TableProvider};
use write_fonts::tables::{head::Head, hhea::Hhea, hmtx, maxp::Maxp, post::Post};
use write_fonts::types::{FWord, GlyphId, GlyphId16, NameId, Tag, Version16Dot16};

use crate::{CompileError, SourceOutlines};

/// TrueType outline, instruction and device tables, meaningless with CFF outlines.
const DROPPED: [Tag; 9] = [
    Tag::new(b"glyf"),
    Tag::new(b"loca"),
    Tag::new(b"cvt "),
    Tag::new(b"fpgm"),
    Tag::new(b"prep"),
    Tag::new(b"hdmx"),
    Tag::new(b"LTSH"),
    Tag::new(b"VDMX"),
    Tag::new(b"gasp"),
];

/// Integer glyph bounds: the exact bounds of the CFF outline, rounded outwards.
#[derive(Debug, Clone, Copy)]
struct Bounds {
    x_min: i16,
    y_min: i16,
    x_max: i16,
    y_max: i16,
}

/// Turns a static TTF compiled by fontc into a CFF-based OTF.
///
/// `outlines` must hold the decomposed cubic outline of every glyph, under the name in
/// the TTF's `post` table. The result keeps the TTF's glyph order and every table
/// except `glyf`, `loca`, `cvt `, `fpgm`, `prep`, `hdmx`, `LTSH`, `VDMX` and `gasp`.
/// It rewrites `maxp` (version 0.5), `post` (version 3.0, same metrics), the `head`
/// bounding box, the `hmtx` left side bearings and the `hhea` extents from the cubic
/// bounds. `OS/2` is kept as is: fontc derives none of its fields from glyph bounds.
/// The sfnt version becomes `OTTO`, and the checksums are recomputed.
pub fn ttf_to_otf(ttf: &[u8], outlines: &SourceOutlines) -> Result<Vec<u8>, CompileError> {
    let font = FontRef::new(ttf)?;
    if font.table_data(Tag::new(b"fvar")).is_some() {
        return Err(transplant("a variable font ships as TTF (ADR-0007)"));
    }
    if font.table_data(Tag::new(b"vmtx")).is_some() {
        return Err(transplant("vertical metrics (vmtx) are not supported yet"));
    }
    let read_hmtx = font.hmtx()?;
    let glyphs = glyphs(&font, &read_hmtx, outlines)?;

    let mut cff = CffBuilder::new(postscript_name(&font)?, font.head()?.units_per_em());
    for glyph in &glyphs {
        cff = cff.glyph(glyph.name, glyph.advance, glyph.path);
    }
    let cff = cff.build()?;
    let bounds = glyphs
        .iter()
        .map(|g| glyph_bounds(g.name, g.path))
        .collect::<Result<Vec<_>, _>>()?;

    let mut builder = FontBuilder::new();
    builder.add_raw(Tag::new(b"CFF "), cff);
    builder.add_table(&head(&font, &bounds)?)?;
    let hhea = hhea(&font, &glyphs, &bounds)?;
    builder.add_table(&hmtx(&glyphs, &bounds, hhea.number_of_h_metrics))?;
    builder.add_table(&hhea)?;
    builder.add_table(&Maxp {
        num_glyphs: u16::try_from(glyphs.len()).map_err(|_| transplant("too many glyphs"))?,
        ..Maxp::default()
    })?;
    builder.add_table(&post(&font)?)?;
    for record in font.table_directory().table_records() {
        let tag = record.tag();
        if DROPPED.contains(&tag) || builder.contains(tag) {
            continue;
        }
        let data = font
            .table_data(tag)
            .ok_or_else(|| transplant(&format!("table '{tag}' is out of bounds")))?;
        builder.add_raw(tag, data.as_bytes());
    }
    Ok(builder.build())
}

fn transplant(reason: &str) -> CompileError {
    CompileError::Transplant(reason.to_owned())
}

struct GlyphSource<'a> {
    name: &'a str,
    advance: u16,
    path: &'a BezPath,
}

/// Every glyph in the TTF's order: its `post` name, advance and source outline.
fn glyphs<'a>(
    font: &FontRef<'a>,
    hmtx: &ReadHmtx,
    outlines: &'a SourceOutlines,
) -> Result<Vec<GlyphSource<'a>>, CompileError> {
    let post = font.post()?;
    (0..font.maxp()?.num_glyphs())
        .map(|gid| {
            let name = post
                .glyph_name(GlyphId16::new(gid))
                .ok_or_else(|| transplant(&format!("glyph {gid} has no name in 'post'")))?;
            let advance = hmtx
                .advance(GlyphId::new(gid.into()))
                .ok_or_else(|| transplant(&format!("glyph {gid} has no advance")))?;
            let path = outlines
                .get(name)
                .ok_or_else(|| CompileError::MissingOutline(name.to_owned()))?;
            Ok(GlyphSource {
                name,
                advance,
                path,
            })
        })
        .collect()
}

/// The PostScript name (name ID 6), preferably the Windows English record.
fn postscript_name(font: &FontRef) -> Result<String, CompileError> {
    let name = font.name()?;
    name.name_record()
        .iter()
        .filter(|record| record.name_id() == NameId::POSTSCRIPT_NAME)
        .max_by_key(|record| (record.platform_id() == 3, record.language_id() == 0x409))
        .and_then(|record| record.string(name.string_data()).ok())
        .map(|string| string.chars().collect())
        .ok_or_else(|| transplant("the font has no PostScript name (name ID 6)"))
}

/// The bounds of the outline the CFF table stores, as fontTools computes them for
/// CFF: exact curve extrema, then floor for the minimum and ceiling for the maximum.
fn glyph_bounds(name: &str, path: &BezPath) -> Result<Option<Bounds>, CompileError> {
    let bounds = tf_cff::bounds(path).map_err(|error| CffError::Glyph {
        name: name.to_owned(),
        error,
    })?;
    bounds
        .map(|b| {
            Ok(Bounds {
                x_min: fword(b.x_min)?,
                y_min: fword(b.y_min)?,
                x_max: fword(b.x_max)?,
                y_max: fword(b.y_max)?,
            })
        })
        .transpose()
}

fn fword(v: i32) -> Result<i16, CompileError> {
    i16::try_from(v).map_err(|_| transplant(&format!("glyph bounds {v} exceed 16 bits")))
}

fn head(font: &FontRef, bounds: &[Option<Bounds>]) -> Result<Head, CompileError> {
    let mut head: Head = font.head()?.to_owned_table();
    let all = bounds.iter().flatten();
    head.x_min = all.clone().map(|b| b.x_min).min().unwrap_or(0);
    head.y_min = all.clone().map(|b| b.y_min).min().unwrap_or(0);
    head.x_max = all.clone().map(|b| b.x_max).max().unwrap_or(0);
    head.y_max = all.map(|b| b.y_max).max().unwrap_or(0);
    Ok(head)
}

/// Left side bearing = the outline's `x_min` (0 without an outline). The advances and
/// the split into long metrics and bare bearings stay as in the TTF.
fn hmtx(glyphs: &[GlyphSource], bounds: &[Option<Bounds>], long_metrics: u16) -> hmtx::Hmtx {
    let lsb = |b: &Option<Bounds>| b.map_or(0, |b| b.x_min);
    let (long, short) = bounds.split_at(usize::from(long_metrics).min(bounds.len()));
    hmtx::Hmtx::new(
        glyphs
            .iter()
            .zip(long)
            .map(|(g, b)| hmtx::LongMetric::new(g.advance, lsb(b)))
            .collect(),
        short.iter().map(lsb).collect(),
    )
}

/// `minLeftSideBearing`, `minRightSideBearing` and `xMaxExtent` over the glyphs with
/// outlines, as fontTools recalculates them; everything else stays as in the TTF.
fn hhea(
    font: &FontRef,
    glyphs: &[GlyphSource],
    bounds: &[Option<Bounds>],
) -> Result<Hhea, CompileError> {
    let mut hhea: Hhea = font.hhea()?.to_owned_table();
    let outlined: Vec<(i32, Bounds)> = glyphs
        .iter()
        .zip(bounds)
        .filter_map(|(g, b)| Some((i32::from(g.advance), (*b)?)))
        .collect();
    let min_lsb = outlined.iter().map(|(_, b)| i32::from(b.x_min)).min();
    let min_rsb = outlined
        .iter()
        .map(|(advance, b)| advance - i32::from(b.x_max))
        .min();
    let max_extent = outlined.iter().map(|(_, b)| i32::from(b.x_max)).max();
    let to_fword = |v: Option<i32>| -> Result<FWord, CompileError> {
        let v = v.unwrap_or(0);
        i16::try_from(v)
            .map(FWord::new)
            .map_err(|_| transplant(&format!("hhea value {v} exceeds 16 bits")))
    };
    hhea.min_left_side_bearing = to_fword(min_lsb)?;
    hhea.min_right_side_bearing = to_fword(min_rsb)?;
    hhea.x_max_extent = to_fword(max_extent)?;
    Ok(hhea)
}

/// Version 3.0: no glyph names (the CFF charset holds them), same metrics.
fn post(font: &FontRef) -> Result<Post, CompileError> {
    let mut post: Post = font.post()?.to_owned_table();
    post.version = Version16Dot16::VERSION_3_0;
    post.num_glyphs = None;
    post.glyph_name_index = None;
    post.string_data = None;
    Ok(post)
}
