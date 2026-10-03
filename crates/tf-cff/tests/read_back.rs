//! Builds small CFF tables with `CffBuilder` and reads them back: read-fonts parses the
//! table and evaluates each charstring, skrifa draws the glyphs from a minimal sfnt.

use kurbo::{BezPath, PathEl};
use read_fonts::ps::cff::CffFontRef;
use read_fonts::ps::cs::CommandSink;
use read_fonts::ps::string::Sid;
use read_fonts::tables::cff::Cff;
use read_fonts::types::{Fixed, GlyphId};
use read_fonts::{FontData, FontRead};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::prelude::{LocationRef, Size};
use skrifa::{FontRef, MetadataProvider};
use tf_cff::{Bounds, CffBuilder, CffError, OutlineError};
use write_fonts::FontBuilder;
use write_fonts::tables::head::Head;
use write_fonts::tables::hhea::Hhea;
use write_fonts::tables::hmtx::{Hmtx, LongMetric};
use write_fonts::tables::maxp::Maxp;

/// A path command with integer coordinates, as read back from a font.
#[derive(Debug, Clone, PartialEq)]
enum Cmd {
    Move(f64, f64),
    Line(f64, f64),
    Curve(f64, f64, f64, f64, f64, f64),
    Close,
}

use Cmd::{Close, Curve, Line, Move};

/// Records the commands of a charstring exactly as read-fonts evaluates them.
#[derive(Default)]
struct Recorder(Vec<Cmd>);

impl CommandSink for Recorder {
    fn move_to(&mut self, x: Fixed, y: Fixed) {
        self.0.push(Move(x.to_f64(), y.to_f64()));
    }
    fn line_to(&mut self, x: Fixed, y: Fixed) {
        self.0.push(Line(x.to_f64(), y.to_f64()));
    }
    fn curve_to(&mut self, cx0: Fixed, cy0: Fixed, cx1: Fixed, cy1: Fixed, x: Fixed, y: Fixed) {
        let f = Fixed::to_f64;
        self.0
            .push(Curve(f(cx0), f(cy0), f(cx1), f(cy1), f(x), f(y)));
    }
    fn close(&mut self) {
        self.0.push(Close);
    }
}

/// Records what skrifa draws.
#[derive(Default)]
struct Pen(Vec<Cmd>);

impl OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.push(Move(x.into(), y.into()));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.push(Line(x.into(), y.into()));
    }
    fn quad_to(&mut self, _cx0: f32, _cy0: f32, _x: f32, _y: f32) {
        panic!("CFF has no quadratic curves");
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.0.push(Curve(
            cx0.into(),
            cy0.into(),
            cx1.into(),
            cy1.into(),
            x.into(),
            y.into(),
        ));
    }
    fn close(&mut self) {
        self.0.push(Close);
    }
}

/// A triangle drawn as a UFO closed contour converts: back to the start point, no
/// `ClosePath`.
fn triangle() -> BezPath {
    let mut path = BezPath::new();
    path.move_to((100.0, 0.0));
    path.line_to((500.0, 0.0));
    path.line_to((300.0, 700.0));
    path.line_to((100.0, 0.0));
    path
}

/// An ellipse-like contour of four cubic curves, closed with `ClosePath`, plus a second
/// subpath, a square without the closing line.
fn curves() -> BezPath {
    let mut path = BezPath::new();
    path.move_to((260.0, -10.0));
    path.curve_to((400.0, -10.0), (500.0, 140.0), (500.0, 350.0));
    path.curve_to((500.0, 560.0), (400.0, 710.0), (260.0, 710.0));
    path.curve_to((120.0, 710.0), (20.0, 560.0), (20.0, 350.0));
    path.curve_to((20.0, 140.0), (120.0, -10.0), (260.0, -10.0));
    path.close_path();
    path.move_to((200.0, 300.0));
    path.line_to((200.0, 400.0));
    path.line_to((320.0, 400.0));
    path.line_to((320.0, 300.0));
    path.close_path();
    path
}

/// Helpers return errors; only `#[test]` functions may unwrap (clippy.toml).
type Res<T> = Result<T, Box<dyn std::error::Error>>;

/// .notdef (width only), `A` (standard string), `a.alt` (custom string).
fn three_glyphs(units_per_em: u16) -> Result<Vec<u8>, CffError> {
    CffBuilder::new("Typefaced-Test", units_per_em)
        .glyph(".notdef", 500, &BezPath::new())
        .glyph("A", 600, &triangle())
        .glyph("a.alt", 520, &curves())
        .build()
}

fn triangle_cmds() -> Vec<Cmd> {
    // The line back to the start is left to the implicit close.
    vec![
        Move(100.0, 0.0),
        Line(500.0, 0.0),
        Line(300.0, 700.0),
        Close,
    ]
}

fn curves_cmds() -> Vec<Cmd> {
    vec![
        Move(260.0, -10.0),
        Curve(400.0, -10.0, 500.0, 140.0, 500.0, 350.0),
        Curve(500.0, 560.0, 400.0, 710.0, 260.0, 710.0),
        Curve(120.0, 710.0, 20.0, 560.0, 20.0, 350.0),
        Curve(20.0, 140.0, 120.0, -10.0, 260.0, -10.0),
        Close,
        Move(200.0, 300.0),
        Line(200.0, 400.0),
        Line(320.0, 400.0),
        Line(320.0, 300.0),
        Close,
    ]
}

/// Evaluates glyph `gid`: its commands and its advance width.
fn evaluate(cff: &[u8], units_per_em: u16, gid: u32) -> Res<(Vec<Cmd>, f64)> {
    let font = CffFontRef::new_cff(cff, 0, Some(units_per_em.into()))?;
    let gid = GlyphId::new(gid);
    let subfont = font.subfont(font.subfont_index(gid).ok_or("no subfont")?, &[])?;
    let mut recorder = Recorder::default();
    let width = font
        .evaluate_charstring(&subfont, gid, &[], &mut recorder)?
        .ok_or("no width")?;
    Ok((recorder.0, width.to_f64()))
}

fn glyph_name(cff: &[u8], gid: u32) -> Res<String> {
    let font = CffFontRef::new_cff(cff, 0, None)?;
    let sid = font
        .charset()
        .ok_or("no charset")?
        .string_id(GlyphId::new(gid))?;
    Ok(String::from_utf8(
        font.string(sid).ok_or("no string")?.to_vec(),
    )?)
}

/// Wraps a `CFF ` table in the smallest sfnt skrifa draws from: `head` for the units
/// per em, `hhea` and `hmtx` for the metrics skrifa loads with the outlines.
fn sfnt(cff: &[u8], units_per_em: u16, num_glyphs: u16) -> Res<Vec<u8>> {
    let head = Head {
        units_per_em,
        ..Default::default()
    };
    let maxp = Maxp {
        num_glyphs,
        ..Default::default()
    };
    let hhea = Hhea {
        number_of_h_metrics: num_glyphs,
        ..Default::default()
    };
    let hmtx = Hmtx::new(vec![LongMetric::new(0, 0); num_glyphs.into()], Vec::new());
    let mut builder = FontBuilder::new();
    builder.add_table(&head)?;
    builder.add_table(&maxp)?;
    builder.add_table(&hhea)?;
    builder.add_table(&hmtx)?;
    builder.add_raw(read_fonts::types::Tag::new(b"CFF "), cff.to_vec());
    Ok(builder.build())
}

fn draw(font: &[u8], gid: u32) -> Res<Vec<Cmd>> {
    let font = FontRef::new(font)?;
    let mut pen = Pen::default();
    font.outline_glyphs()
        .get(GlyphId::new(gid))
        .ok_or("no outline")?
        .draw(
            DrawSettings::unhinted(Size::unscaled(), LocationRef::default()),
            &mut pen,
        )?;
    Ok(pen.0)
}

#[test]
fn the_name_index_holds_the_font_name() {
    let cff = three_glyphs(1000).unwrap();

    let table = Cff::read(FontData::new(&cff)).unwrap();

    assert_eq!(table.header().major(), 1);
    assert_eq!(table.names().count(), 1);
    assert_eq!(table.names().get(0).unwrap(), b"Typefaced-Test");
}

#[test]
fn standard_names_use_their_standard_sid_and_custom_names_start_at_391() {
    let cff = three_glyphs(1000).unwrap();
    let font = CffFontRef::new_cff(&cff, 0, None).unwrap();
    let charset = font.charset().unwrap();

    let sid = |gid| charset.string_id(GlyphId::new(gid)).unwrap().to_u16();

    assert_eq!(font.num_glyphs(), 3);
    assert_eq!([sid(0), sid(1), sid(2)], [0, 34, 391]);
    assert_eq!(Sid::new(34).resolve_standard(), Ok(&b"A"[..]));
    assert_eq!(font.strings().unwrap().count(), 1);
    assert_eq!(glyph_name(&cff, 2).unwrap(), "a.alt");
}

#[test]
fn a_notdef_glyph_with_only_a_width_has_no_commands() {
    let cff = three_glyphs(1000).unwrap();

    assert_eq!(evaluate(&cff, 1000, 0).unwrap(), (Vec::new(), 500.0));
}

#[test]
fn every_glyph_evaluates_to_the_source_points_and_advance() {
    let cff = three_glyphs(1000).unwrap();

    assert_eq!(evaluate(&cff, 1000, 1).unwrap(), (triangle_cmds(), 600.0));
    assert_eq!(evaluate(&cff, 1000, 2).unwrap(), (curves_cmds(), 520.0));
}

#[test]
fn skrifa_draws_identical_points() {
    let font = sfnt(&three_glyphs(1000).unwrap(), 1000, 3).unwrap();

    assert_eq!(draw(&font, 0).unwrap(), Vec::new());
    assert_eq!(draw(&font, 1).unwrap(), triangle_cmds());
    assert_eq!(draw(&font, 2).unwrap(), curves_cmds());
}

#[test]
fn units_per_em_1000_uses_the_default_font_matrix() {
    let cff = three_glyphs(1000).unwrap();

    assert!(
        CffFontRef::new_cff(&cff, 0, None)
            .unwrap()
            .matrix()
            .is_none()
    );
}

#[test]
fn units_per_em_2048_writes_a_font_matrix_and_still_draws_unscaled() {
    let cff = three_glyphs(2048).unwrap();

    assert!(
        CffFontRef::new_cff(&cff, 0, None)
            .unwrap()
            .matrix()
            .is_some()
    );
    let font = sfnt(&cff, 2048, 3).unwrap();
    assert_eq!(draw(&font, 1).unwrap(), triangle_cmds());
    assert_eq!(draw(&font, 2).unwrap(), curves_cmds());
}

#[test]
fn the_most_common_advance_becomes_the_default_width() {
    let cff = CffBuilder::new("Widths", 1000)
        .glyph(".notdef", 500, &BezPath::new())
        .glyph("a", 600, &triangle())
        .glyph("b", 600, &BezPath::new())
        .glyph("c", 1200, &BezPath::new())
        .build()
        .unwrap();
    let font = CffFontRef::new_cff(&cff, 0, None).unwrap();
    let subfont = font.subfont(0, &[]).unwrap();

    assert_eq!(subfont.default_width(), Some(Fixed::from_i32(600)));
    assert_eq!(subfont.nominal_width(), Fixed::from_i32(600));
    let widths: Vec<f64> = (0..4)
        .map(|gid| evaluate(&cff, 1000, gid).unwrap().1)
        .collect();
    assert_eq!(widths, [500.0, 600.0, 600.0, 1200.0]);
    // A glyph at the default width stores no width: `b` is a bare endchar.
    assert_eq!(font.charstrings().get(2).unwrap(), [14]);
}

#[test]
fn points_are_rounded_half_up_without_drift() {
    let mut path = BezPath::new();
    path.move_to((10.5, -10.5));
    // Steps of 0.4 would drift if the deltas were rounded instead of the points.
    path.line_to((10.9, 0.4));
    path.line_to((11.3, 0.8));
    path.line_to((11.7, 1.2));
    path.curve_to((20.49, 30.5), (-0.5, 2.5), (0.0, 0.0));
    let cff = CffBuilder::new("Rounding", 1000)
        .glyph(".notdef", 0, &path)
        .build()
        .unwrap();

    let (cmds, _) = evaluate(&cff, 1000, 0).unwrap();

    assert_eq!(
        cmds,
        [
            Move(11.0, -10.0),
            Line(11.0, 0.0),
            Line(11.0, 1.0),
            Line(12.0, 1.0),
            Curve(20.0, 31.0, 0.0, 3.0, 0.0, 0.0),
            Close,
        ]
    );
}

#[test]
fn subpaths_without_segments_are_left_out() {
    let mut path = BezPath::new();
    path.move_to((5.0, 5.0));
    path.move_to((1.0, 1.0));
    path.line_to((1.0, 1.0)); // a line back to the start only
    path.move_to((0.0, 0.0));
    path.line_to((10.0, 0.0));
    path.line_to((10.0, 10.0));
    let cff = CffBuilder::new("Empty", 1000)
        .glyph(".notdef", 0, &path)
        .build()
        .unwrap();

    let (cmds, _) = evaluate(&cff, 1000, 0).unwrap();

    assert_eq!(
        cmds,
        [Move(0.0, 0.0), Line(10.0, 0.0), Line(10.0, 10.0), Close]
    );
}

#[test]
fn the_font_bbox_is_the_union_of_the_exact_curve_bounds_rounded_outwards() {
    let mut bump = BezPath::new();
    bump.move_to((0.0, 0.0));
    bump.curve_to((0.0, 101.0), (100.0, 101.0), (100.0, 0.0)); // top at y = 75.75
    let cff = CffBuilder::new("Bounds", 1000)
        .glyph(".notdef", 0, &BezPath::new())
        .glyph("a", 0, &bump)
        .glyph("b", 0, &triangle())
        .build()
        .unwrap();

    let bbox = CffFontRef::new_cff(&cff, 0, None)
        .unwrap()
        .metadata()
        .unwrap()
        .bbox();

    let ints = [bbox.x_min, bbox.y_min, bbox.x_max, bbox.y_max].map(Fixed::to_i32);
    assert_eq!(ints, [0, 0, 500, 700]);
}

#[test]
fn bounds_are_those_of_the_rounded_outline_rounded_outwards() {
    let mut bump = BezPath::new();
    bump.move_to((0.4, 0.0)); // rounds to 0
    bump.curve_to((0.0, 101.0), (100.0, 101.0), (100.0, 0.0)); // top at y = 75.75
    bump.line_to((0.4, 0.0));
    bump.move_to((900.0, 900.0)); // draws nothing

    let bounds = tf_cff::bounds(&bump).unwrap();

    let expected = Bounds {
        x_min: 0,
        y_min: 0,
        x_max: 100,
        y_max: 76,
    };
    assert_eq!(bounds, Some(expected));
    assert_eq!(tf_cff::bounds(&BezPath::new()).unwrap(), None);
}

#[test]
fn bounds_ignore_float_noise_at_a_vertical_end_tangent() {
    // From Inria Sans Bold `two`: the curve ends at x = 40 with a vertical tangent;
    // kurbo puts the extremum a hair below 40, which floor would make 39.
    let mut path = BezPath::new();
    path.move_to((180.0, 346.0));
    path.curve_to((103.0, 308.0), (40.0, 238.0), (40.0, 95.0));
    path.line_to((300.0, 95.0));

    let bounds = tf_cff::bounds(&path).unwrap().unwrap();

    assert_eq!(bounds.x_min, 40);
}

#[test]
fn bounds_of_the_union() {
    let a = Bounds {
        x_min: 0,
        y_min: -5,
        x_max: 10,
        y_max: 10,
    };
    let b = Bounds {
        x_min: -3,
        y_min: 0,
        x_max: 5,
        y_max: 20,
    };

    let expected = Bounds {
        x_min: -3,
        y_min: -5,
        x_max: 10,
        y_max: 20,
    };
    assert_eq!(a.union(b), expected);
}

fn build_error(builder: CffBuilder) -> CffError {
    match builder.build() {
        Ok(_) => panic!("the build should fail"),
        Err(error) => error,
    }
}

#[test]
fn the_first_glyph_must_be_notdef() {
    let err = build_error(CffBuilder::new("F", 1000).glyph("A", 0, &BezPath::new()));
    assert_eq!(err, CffError::NotdefNotFirst);
    assert_eq!(
        build_error(CffBuilder::new("F", 1000)),
        CffError::NotdefNotFirst
    );
}

#[test]
fn glyph_names_must_be_unique_and_printable() {
    let notdef = || CffBuilder::new("F", 1000).glyph(".notdef", 0, &BezPath::new());

    let dup = build_error(
        notdef()
            .glyph("a", 0, &BezPath::new())
            .glyph("a", 0, &BezPath::new()),
    );
    let space = build_error(notdef().glyph("a b", 0, &BezPath::new()));
    let empty = build_error(notdef().glyph("", 0, &BezPath::new()));
    let second_notdef = build_error(notdef().glyph(".notdef", 0, &BezPath::new()));

    assert_eq!(dup, CffError::DuplicateGlyphName("a".into()));
    assert_eq!(space, CffError::InvalidGlyphName("a b".into()));
    assert_eq!(empty, CffError::InvalidGlyphName(String::new()));
    assert_eq!(
        second_notdef,
        CffError::DuplicateGlyphName(".notdef".into())
    );
}

#[test]
fn the_font_name_must_be_a_valid_postscript_name() {
    for name in [
        "",
        "Has Space",
        "Paren(",
        "Slash/",
        "Ünicode",
        &"x".repeat(64),
    ] {
        let err = build_error(CffBuilder::new(name, 1000).glyph(".notdef", 0, &BezPath::new()));
        assert_eq!(err, CffError::InvalidFontName(name.to_owned()), "{name:?}");
    }
    assert!(
        CffBuilder::new("x".repeat(63), 1000)
            .glyph(".notdef", 0, &BezPath::new())
            .build()
            .is_ok()
    );
}

#[test]
fn units_per_em_must_be_in_the_opentype_range() {
    for upem in [0, 15, 16385] {
        let err = build_error(CffBuilder::new("F", upem).glyph(".notdef", 0, &BezPath::new()));
        assert_eq!(err, CffError::InvalidUnitsPerEm(upem));
    }
}

fn outline_error(path: &BezPath) -> CffError {
    build_error(CffBuilder::new("F", 1000).glyph(".notdef", 0, path))
}

fn glyph_error(error: OutlineError) -> CffError {
    CffError::Glyph {
        name: ".notdef".into(),
        error,
    }
}

#[test]
fn quadratic_segments_are_rejected() {
    let mut path = BezPath::new();
    path.move_to((0.0, 0.0));
    path.quad_to((50.0, 100.0), (100.0, 0.0));

    assert_eq!(outline_error(&path), glyph_error(OutlineError::Quadratic));
}

#[test]
fn a_path_must_start_with_a_move() {
    // kurbo only debug-asserts the leading move; `elements_mut` gets past it.
    let mut path = BezPath::new();
    path.move_to((0.0, 0.0));
    path.elements_mut()[0] = PathEl::LineTo((10.0, 0.0).into());

    assert_eq!(outline_error(&path), glyph_error(OutlineError::MissingMove));
}

#[test]
fn a_segment_right_after_close_starts_at_the_previous_start_as_in_kurbo() {
    let mut path = BezPath::new();
    path.move_to((0.0, 0.0));
    path.line_to((10.0, 0.0));
    path.line_to((10.0, 10.0));
    path.close_path();
    path.line_to((-10.0, 0.0));
    path.line_to((-10.0, -10.0));
    let cff = CffBuilder::new("F", 1000)
        .glyph(".notdef", 0, &path)
        .build()
        .unwrap();

    let (cmds, _) = evaluate(&cff, 1000, 0).unwrap();

    assert_eq!(
        cmds,
        [
            Move(0.0, 0.0),
            Line(10.0, 0.0),
            Line(10.0, 10.0),
            Close,
            Move(0.0, 0.0),
            Line(-10.0, 0.0),
            Line(-10.0, -10.0),
            Close,
        ]
    );
}

#[test]
fn non_finite_coordinates_are_rejected() {
    let mut path = BezPath::new();
    path.move_to((f64::NAN, 0.0));

    assert_eq!(outline_error(&path), glyph_error(OutlineError::NonFinite));
}

#[test]
fn a_charstring_longer_than_65535_bytes_is_rejected() {
    // TN #5177 Appendix B: a charstring holds at most 65535 bytes.
    let mut path = BezPath::new();
    path.move_to((0.0, 0.0));
    for i in 1..=10_000 {
        let v = if i % 2 == 1 { 2000.0 } else { 0.0 };
        path.line_to((v, v));
    }

    // 0 0 rmoveto (3 bytes), 9999 lines of ±2000 ±2000 rlineto (7 bytes each; the
    // last line returns to the start and is dropped), endchar (1 byte).
    assert_eq!(
        outline_error(&path),
        glyph_error(OutlineError::TooLong(3 + 9999 * 7 + 1))
    );
}

#[test]
fn numbers_beyond_16_bits_are_rejected() {
    let mut path = BezPath::new();
    path.move_to((-20000.0, 0.0));
    path.line_to((20000.0, 0.0));
    let wide = build_error(CffBuilder::new("F", 1000).glyph(".notdef", 0, &path));
    let width = build_error(
        CffBuilder::new("F", 1000)
            .glyph(".notdef", 0, &BezPath::new())
            .glyph("a", 0, &BezPath::new())
            .glyph("b", 40000, &BezPath::new()),
    );

    assert_eq!(wide, glyph_error(OutlineError::OutOfRange(40000)));
    assert_eq!(
        width,
        CffError::Glyph {
            name: "b".into(),
            error: OutlineError::OutOfRange(40000)
        }
    );
}

#[test]
fn more_than_65535_glyphs_are_rejected() {
    let mut builder = CffBuilder::new("F", 1000).glyph(".notdef", 0, &BezPath::new());
    for i in 1..65536 {
        builder = builder.glyph(format!("g{i}"), 0, &BezPath::new());
    }

    assert_eq!(build_error(builder), CffError::TooManyGlyphs(65536));
}

#[test]
fn custom_names_beyond_sid_64999_are_rejected() {
    // TN #5176 Table 2: SIDs are 0–64999. With 391 standard strings, custom names may
    // use SIDs 391..=64999, 64609 of them.
    let with_custom_names = |count: u32| {
        let mut builder = CffBuilder::new("F", 1000).glyph(".notdef", 0, &BezPath::new());
        for i in 0..count {
            builder = builder.glyph(format!("g{i}"), 0, &BezPath::new());
        }
        builder.build()
    };

    assert!(with_custom_names(64609).is_ok());
    assert_eq!(
        with_custom_names(64610).err(),
        Some(CffError::TooManyStrings(64610))
    );
}
