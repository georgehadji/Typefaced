//! TTF → OTF transplant (ADR-0007): converts the fixture UFO, and with the corpus the
//! static family, then checks the OTF with read-fonts and skrifa against the cubic
//! source and against fontc's TTF. Corpus tests need `cargo xtask corpus fetch`, then
//! `cargo test -p tf-compile -- --ignored`.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use kurbo::{BezPath, PathEl, Point, Shape};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::prelude::{LocationRef, Size};
use skrifa::raw::ps::cff::CffFontRef;
use skrifa::raw::tables::cff::Cff;
use skrifa::raw::types::{GlyphId, Tag, Version16Dot16};
use skrifa::raw::{FontData, FontRead, TableProvider};
use skrifa::{FontRef, MetadataProvider};
use tf_compile::{CompileError, SourceOutlines, compile_to_otf, compile_to_ttf, ttf_to_otf};
use write_fonts::FontBuilder;

type Res<T> = Result<T, Box<dyn std::error::Error>>;

const DROPPED: [&[u8; 4]; 9] = [
    b"glyf", b"loca", b"cvt ", b"fpgm", b"prep", b"hdmx", b"LTSH", b"VDMX", b"gasp",
];
/// Tables that do not depend on outlines: copied byte for byte.
const KEPT: [&[u8; 4]; 7] = [
    b"cmap", b"name", b"OS/2", b"GSUB", b"GPOS", b"GDEF", b"STAT",
];

/// A converted font: fontc's TTF, the OTF made from it and the source outlines.
struct Converted {
    ttf: Vec<u8>,
    otf: Vec<u8>,
    outlines: SourceOutlines,
}

fn convert(ufo: &Path) -> Res<Converted> {
    let ttf = compile_to_ttf(ufo)?;
    let outlines = SourceOutlines::from_ufo(ufo)?;
    let otf = ttf_to_otf(&ttf, &outlines)?;
    Ok(Converted { ttf, otf, outlines })
}

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/min.ufo")
}

/// The fixture converted once for all tests.
fn fixture() -> &'static Converted {
    static FIXTURE: OnceLock<Converted> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        convert(&fixture_path()).unwrap_or_else(|e| panic!("fixture conversion failed: {e}"))
    })
}

fn corpus(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/.cache")
        .join(relative)
}

/// A path command, as drawn by skrifa.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Cmd {
    Move(f64, f64),
    Line(f64, f64),
    Curve(f64, f64, f64, f64, f64, f64),
    Close,
}

#[derive(Default)]
struct Pen(Vec<Cmd>);

impl OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.push(Cmd::Move(x.into(), y.into()));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.push(Cmd::Line(x.into(), y.into()));
    }
    fn quad_to(&mut self, _cx0: f32, _cy0: f32, _x: f32, _y: f32) {
        panic!("CFF has no quadratic curves");
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let f = f64::from;
        self.0
            .push(Cmd::Curve(f(cx0), f(cy0), f(cx1), f(cy1), f(x), f(y)));
    }
    fn close(&mut self) {
        self.0.push(Cmd::Close);
    }
}

fn draw(font: &FontRef, gid: GlyphId) -> Res<Vec<Cmd>> {
    let mut pen = Pen::default();
    font.outline_glyphs().get(gid).ok_or("no outline")?.draw(
        DrawSettings::unhinted(Size::unscaled(), LocationRef::default()),
        &mut pen,
    )?;
    Ok(pen.0)
}

/// OpenType rounding, as fontc and fontTools do it.
fn ot_round(p: Point) -> (f64, f64) {
    ((p.x + 0.5).floor(), (p.y + 0.5).floor())
}

/// What skrifa should draw for a source path: points rounded, and the clean-up skrifa
/// applies to every CFF outline (as FreeType does): zero-length lines and a final line
/// back to the start are not drawn, subpaths without segments are not drawn, and every
/// subpath is closed.
fn expected_cmds(path: &BezPath) -> Vec<Cmd> {
    fn flush(out: &mut Vec<Cmd>, start: (f64, f64), segments: &mut Vec<Cmd>) {
        if segments.last() == Some(&Cmd::Line(start.0, start.1)) {
            segments.pop();
        }
        if !segments.is_empty() {
            out.push(Cmd::Move(start.0, start.1));
            out.append(segments);
            out.push(Cmd::Close);
        }
        segments.clear();
    }
    let mut out = Vec::new();
    let mut segments = Vec::new();
    let (mut start, mut current) = ((0.0, 0.0), (0.0, 0.0));
    for element in path.elements() {
        match *element {
            PathEl::MoveTo(p) => {
                flush(&mut out, start, &mut segments);
                start = ot_round(p);
                current = start;
            }
            PathEl::LineTo(p) => {
                let p = ot_round(p);
                if p != current {
                    segments.push(Cmd::Line(p.0, p.1));
                    current = p;
                }
            }
            PathEl::CurveTo(a, b, c) => {
                let (a, b, c) = (ot_round(a), ot_round(b), ot_round(c));
                segments.push(Cmd::Curve(a.0, a.1, b.0, b.1, c.0, c.1));
                current = c;
            }
            PathEl::QuadTo(..) => panic!("the sources are cubic"),
            PathEl::ClosePath => {
                flush(&mut out, start, &mut segments);
                current = start;
            }
        }
    }
    flush(&mut out, start, &mut segments);
    out
}

/// The number of commands that differ between two outlines (a missing or extra command
/// counts as one).
fn differences(a: &[Cmd], b: &[Cmd]) -> usize {
    let differing = a.iter().zip(b).filter(|(x, y)| x != y).count();
    differing + a.len().abs_diff(b.len())
}

/// Glyph names in the TTF's order (from its `post` table).
fn ttf_glyph_names(ttf: &FontRef) -> Res<Vec<String>> {
    let count = ttf.maxp()?.num_glyphs();
    (0..u32::from(count))
        .map(|gid| {
            ttf.glyph_names()
                .get(GlyphId::new(gid))
                .map(|name| name.as_str().to_owned())
                .ok_or_else(|| format!("glyph {gid} has no name").into())
        })
        .collect()
}

/// Compares every OTF glyph with its rounded, decomposed source outline. Returns the
/// number of glyphs, the number of commands compared and the differing glyphs.
fn compare_outlines(font: &Converted) -> Res<(usize, usize, Vec<String>)> {
    let ttf = FontRef::new(&font.ttf)?;
    let otf = FontRef::new(&font.otf)?;
    let mut compared = 0;
    let mut differing = Vec::new();
    for (gid, name) in ttf_glyph_names(&ttf)?.iter().enumerate() {
        let source = font.outlines.get(name).ok_or("no source outline")?;
        let expected = expected_cmds(source);
        let drawn = draw(&otf, GlyphId::new(u32::try_from(gid)?))?;
        compared += expected.len();
        if differences(&expected, &drawn) > 0 {
            differing.push(format!("{name}: expected {expected:?}, drawn {drawn:?}"));
        }
    }
    Ok((ttf_glyph_names(&ttf)?.len(), compared, differing))
}

/// Integer bounds `(x_min, y_min, x_max, y_max)` of a rounded source outline, rounded
/// outwards; `None` without segments. Computed here from the source, not by tf-cff.
/// Extrema within 1e-6 of an integer are that integer: float noise, not geometry
/// (fontTools agrees on the corpus; see the spike report).
fn source_bounds(path: &BezPath) -> Option<[i32; 4]> {
    let rounded = BezPath::from_vec(
        path.elements()
            .iter()
            .map(|el| {
                let r = |p: Point| {
                    let (x, y) = ot_round(p);
                    Point::new(x, y)
                };
                match *el {
                    PathEl::MoveTo(p) => PathEl::MoveTo(r(p)),
                    PathEl::LineTo(p) => PathEl::LineTo(r(p)),
                    PathEl::CurveTo(a, b, c) => PathEl::CurveTo(r(a), r(b), r(c)),
                    other => other,
                }
            })
            .collect(),
    );
    rounded.segments().next()?;
    let b = rounded.bounding_box();
    let snap = |v: f64| {
        if (v - v.round()).abs() < 1e-6 {
            v.round()
        } else {
            v
        }
    };
    let outward = [
        snap(b.x0).floor(),
        snap(b.y0).floor(),
        snap(b.x1).ceil(),
        snap(b.y1).ceil(),
    ];
    Some(outward.map(|v| v as i32))
}

/// Checks `head`, `hmtx` and `hhea` against the bounds of the source outlines.
fn check_bounds(font: &Converted) -> Res<()> {
    let ttf = FontRef::new(&font.ttf)?;
    let otf = FontRef::new(&font.otf)?;
    let hmtx = otf.hmtx()?;
    let mut union: Option<[i32; 4]> = None;
    let (mut min_lsb, mut min_rsb, mut max_extent) = (i32::MAX, i32::MAX, i32::MIN);
    for (gid, name) in ttf_glyph_names(&ttf)?.iter().enumerate() {
        let gid = GlyphId::new(u32::try_from(gid)?);
        let lsb = i32::from(hmtx.side_bearing(gid).ok_or("no lsb")?);
        let advance = i32::from(hmtx.advance(gid).ok_or("no advance")?);
        match source_bounds(font.outlines.get(name).ok_or("no source outline")?) {
            None => assert_eq!(lsb, 0, "{name}"),
            Some(b) => {
                assert_eq!(lsb, b[0], "{name} lsb");
                min_lsb = min_lsb.min(lsb);
                min_rsb = min_rsb.min(advance - b[2]);
                max_extent = max_extent.max(b[2]);
                union = Some(union.map_or(b, |u| {
                    [
                        u[0].min(b[0]),
                        u[1].min(b[1]),
                        u[2].max(b[2]),
                        u[3].max(b[3]),
                    ]
                }));
            }
        }
    }
    let head = otf.head()?;
    let head_box = [head.x_min(), head.y_min(), head.x_max(), head.y_max()].map(i32::from);
    assert_eq!(Some(head_box), union, "head bounding box");
    let hhea = otf.hhea()?;
    assert_eq!(i32::from(hhea.min_left_side_bearing().to_i16()), min_lsb);
    assert_eq!(i32::from(hhea.min_right_side_bearing().to_i16()), min_rsb);
    assert_eq!(i32::from(hhea.x_max_extent().to_i16()), max_extent);
    Ok(())
}

fn check_tables(font: &Converted) -> Res<()> {
    let ttf = FontRef::new(&font.ttf)?;
    let otf = FontRef::new(&font.otf)?;
    assert_eq!(
        otf.table_directory().sfnt_version(),
        u32::from_be_bytes(*b"OTTO")
    );
    assert!(otf.table_data(Tag::new(b"CFF ")).is_some());
    for tag in DROPPED {
        assert!(otf.table_data(Tag::new(tag)).is_none(), "{tag:?} kept");
    }
    for tag in KEPT {
        let tag = Tag::new(tag);
        assert_eq!(
            ttf.table_data(tag).map(|d| d.as_bytes()),
            otf.table_data(tag).map(|d| d.as_bytes()),
            "{tag} changed"
        );
    }
    let maxp = otf.maxp()?;
    assert_eq!(maxp.version(), Version16Dot16::VERSION_0_5);
    assert_eq!(maxp.num_glyphs(), ttf.maxp()?.num_glyphs());
    let (post, ttf_post) = (otf.post()?, ttf.post()?);
    assert_eq!(post.version(), Version16Dot16::VERSION_3_0);
    assert_eq!(post.italic_angle(), ttf_post.italic_angle());
    assert_eq!(post.underline_position(), ttf_post.underline_position());
    assert_eq!(post.underline_thickness(), ttf_post.underline_thickness());
    assert_eq!(post.is_fixed_pitch(), ttf_post.is_fixed_pitch());
    Ok(())
}

fn check_advances(font: &Converted) -> Res<()> {
    let (ttf, otf) = (FontRef::new(&font.ttf)?, FontRef::new(&font.otf)?);
    let (ttf_hmtx, otf_hmtx) = (ttf.hmtx()?, otf.hmtx()?);
    assert_eq!(ttf_hmtx.h_metrics().len(), otf_hmtx.h_metrics().len());
    for gid in 0..u32::from(ttf.maxp()?.num_glyphs()) {
        let gid = GlyphId::new(gid);
        assert_eq!(ttf_hmtx.advance(gid), otf_hmtx.advance(gid), "{gid}");
    }
    Ok(())
}

fn check_checksums(otf: &[u8]) -> Res<()> {
    use skrifa::raw::tables::compute_checksum;
    assert_eq!(
        compute_checksum(otf),
        0xB1B0_AFBA,
        "head.checkSumAdjustment"
    );
    let font = FontRef::new(otf)?;
    for record in font.table_directory().table_records() {
        let mut data = font
            .table_data(record.tag())
            .ok_or("no data")?
            .as_bytes()
            .to_vec();
        if record.tag() == Tag::new(b"head") {
            data[8..12].fill(0); // checkSumAdjustment is left out of head's own checksum
        }
        assert_eq!(
            compute_checksum(&data),
            record.checksum(),
            "{}",
            record.tag()
        );
    }
    Ok(())
}

/// The Name INDEX holds the PostScript name, and the charset the TTF's glyph names.
fn check_cff_names(font: &Converted) -> Res<()> {
    let ttf = FontRef::new(&font.ttf)?;
    let otf = FontRef::new(&font.otf)?;
    let cff_data = otf.table_data(Tag::new(b"CFF ")).ok_or("no CFF")?;
    let ps_name = ttf
        .localized_strings(skrifa::string::StringId::POSTSCRIPT_NAME)
        .english_or_first()
        .ok_or("no PostScript name")?
        .to_string();
    let names = Cff::read(FontData::new(cff_data.as_bytes()))?.names();
    assert_eq!(names.count(), 1);
    assert_eq!(names.get(0)?, ps_name.as_bytes());
    let cff = CffFontRef::new_cff(cff_data.as_bytes(), 0, None)?;
    let charset = cff.charset().ok_or("no charset")?;
    for (gid, name) in ttf_glyph_names(&ttf)?.iter().enumerate() {
        let sid = charset.string_id(GlyphId::new(u32::try_from(gid)?))?;
        assert_eq!(cff.string(sid), Some(name.as_bytes()), "glyph {gid}");
    }
    Ok(())
}

#[test]
fn the_otf_has_cff_outlines_and_keeps_the_other_tables() {
    check_tables(fixture()).unwrap();
}

#[test]
fn every_glyph_draws_the_rounded_decomposed_source_points() {
    let (glyphs, compared, differing) = compare_outlines(fixture()).unwrap();

    assert_eq!(glyphs, 6);
    assert!(compared > 0);
    assert_eq!(differing, Vec::<String>::new());
}

#[test]
fn aacute_is_a_plus_acute_moved_by_the_component_offset() {
    let otf = FontRef::new(&fixture().otf).unwrap();
    let glyph = |c: char| otf.charmap().map(c).unwrap();
    let a = draw(&otf, glyph('A')).unwrap();
    let acute = draw(&otf, glyph('\u{B4}')).unwrap();

    let moved = acute.iter().map(|cmd| match *cmd {
        Cmd::Move(x, y) => Cmd::Move(x + 120.0, y + 200.0),
        Cmd::Line(x, y) => Cmd::Line(x + 120.0, y + 200.0),
        Cmd::Curve(a, b, c, d, e, f) => Cmd::Curve(
            a + 120.0,
            b + 200.0,
            c + 120.0,
            d + 200.0,
            e + 120.0,
            f + 200.0,
        ),
        Cmd::Close => Cmd::Close,
    });
    let expected: Vec<Cmd> = a.iter().copied().chain(moved).collect();

    assert_eq!(draw(&otf, glyph('\u{C1}')).unwrap(), expected);
}

#[test]
fn advances_are_the_ttfs() {
    check_advances(fixture()).unwrap();
}

#[test]
fn head_hhea_and_hmtx_bounds_match_the_cubic_outlines() {
    check_bounds(fixture()).unwrap();
}

#[test]
fn table_checksums_and_the_font_checksum_are_valid() {
    check_checksums(&fixture().otf).unwrap();
}

#[test]
fn the_cff_names_match_the_ttf() {
    check_cff_names(fixture()).unwrap();
}

#[test]
fn compile_to_otf_is_the_ttf_transplanted() {
    assert_eq!(compile_to_otf(&fixture_path()).unwrap(), fixture().otf);
}

#[test]
fn a_glyph_without_a_source_outline_is_an_error() {
    let outlines: SourceOutlines = fixture()
        .outlines
        .iter()
        .filter(|(name, _)| *name != "acute")
        .map(|(name, path)| (name.to_owned(), path.clone()))
        .collect();

    let err = ttf_to_otf(&fixture().ttf, &outlines).unwrap_err();

    assert!(
        matches!(err, CompileError::MissingOutline(ref name) if name == "acute"),
        "{err}"
    );
}

#[test]
fn a_variable_ttf_is_rejected() {
    let ttf = FontRef::new(&fixture().ttf).unwrap();
    let mut builder = FontBuilder::new();
    builder.add_raw(Tag::new(b"fvar"), vec![0; 16]);
    builder.copy_missing_tables(ttf);
    let variable = builder.build();

    let err = ttf_to_otf(&variable, &fixture().outlines).unwrap_err();

    assert!(matches!(err, CompileError::Transplant(_)), "{err}");
    assert!(err.to_string().contains("variable"), "{err}");
}

#[test]
fn a_designspace_source_cannot_become_an_otf() {
    let designspace = Path::new("family.designspace");

    let err = compile_to_otf(designspace).unwrap_err();

    assert!(matches!(err, CompileError::Transplant(_)), "{err}");
}

#[test]
fn bytes_that_are_not_a_font_are_an_error() {
    let err = ttf_to_otf(b"not a font", &fixture().outlines).unwrap_err();

    assert!(matches!(err, CompileError::Read(_)), "{err}");
}

const STATIC_STYLES: [&str; 6] = [
    "Regular",
    "Italic",
    "Light",
    "LightItalic",
    "Bold",
    "BoldItalic",
];

#[test]
#[ignore = "needs corpus: cargo xtask corpus fetch"]
fn every_style_of_the_static_family_converts_with_identical_outlines() {
    for style in STATIC_STYLES {
        let ufo = corpus(&format!("inria-sans/InriaSans-{style}.ufo"));
        let font = convert(&ufo).unwrap_or_else(|e| panic!("{style}: {e}"));

        let (glyphs, compared, differing) = compare_outlines(&font).unwrap();
        eprintln!(
            "{style}: {glyphs} glyphs, {compared} commands compared, {} differing glyphs",
            differing.len()
        );
        assert_eq!(differing, Vec::<String>::new(), "{style}");
        check_tables(&font).unwrap();
        check_advances(&font).unwrap();
        check_bounds(&font).unwrap();
        check_checksums(&font.otf).unwrap();
        check_cff_names(&font).unwrap();
    }
}
