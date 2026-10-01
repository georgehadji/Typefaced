//! Compiles the hand-made fixture UFO in-process and checks the TTF with fontations.

use std::path::{Path, PathBuf};

use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::prelude::{LocationRef, Size};
use skrifa::raw::TableProvider;
use skrifa::{FontRef, GlyphId, MetadataProvider, Tag};
use tf_compile::{CompileError, compile_to_ttf};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/min.ufo")
}

fn glyph_name(font: &FontRef, gid: GlyphId) -> String {
    font.glyph_names()
        .get(gid)
        .map(|name| name.as_str().to_owned())
        .unwrap_or_default()
}

/// Counts contours by counting `move_to` calls.
#[derive(Default)]
struct ContourCounter(usize);

impl OutlinePen for ContourCounter {
    fn move_to(&mut self, _x: f32, _y: f32) {
        self.0 += 1;
    }
    fn line_to(&mut self, _x: f32, _y: f32) {}
    fn quad_to(&mut self, _cx0: f32, _cy0: f32, _x: f32, _y: f32) {}
    fn curve_to(&mut self, _cx0: f32, _cy0: f32, _cx1: f32, _cy1: f32, _x: f32, _y: f32) {}
    fn close(&mut self) {}
}

#[test]
fn the_fixture_ufo_compiles_to_a_ttf_with_its_six_glyphs() {
    let bytes = compile_to_ttf(&fixture()).unwrap();
    let font = FontRef::new(&bytes).unwrap();

    assert_eq!(font.maxp().unwrap().num_glyphs(), 6);
    assert!(font.table_data(Tag::new(b"glyf")).is_some());
}

#[test]
fn cmap_maps_code_points_to_the_source_glyphs() {
    let bytes = compile_to_ttf(&fixture()).unwrap();
    let font = FontRef::new(&bytes).unwrap();
    let cmap = font.charmap();

    let a = cmap.map('A').unwrap();
    let aacute = cmap.map('\u{C1}').unwrap();

    assert_eq!(glyph_name(&font, a), "A");
    assert_eq!(glyph_name(&font, aacute), "Aacute");
}

#[test]
fn a_draws_with_the_same_two_contours_as_the_source() {
    let bytes = compile_to_ttf(&fixture()).unwrap();
    let font = FontRef::new(&bytes).unwrap();
    let a = font.charmap().map('A').unwrap();
    let mut pen = ContourCounter::default();

    font.outline_glyphs()
        .get(a)
        .unwrap()
        .draw(
            DrawSettings::unhinted(Size::unscaled(), LocationRef::default()),
            &mut pen,
        )
        .unwrap();

    assert_eq!(pen.0, 2);
}

#[test]
fn other_source_formats_are_rejected() {
    let glyphs = Path::new("font.glyphs");

    let err = compile_to_ttf(glyphs).unwrap_err();

    assert!(matches!(err, CompileError::UnsupportedSource(ref p) if p == glyphs));
    assert!(err.to_string().contains("font.glyphs"));
}

#[test]
fn a_panic_inside_fontc_becomes_an_error() {
    let designspace =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/discrete-axis.designspace");

    let err = compile_to_ttf(&designspace).unwrap_err();

    assert!(matches!(err, CompileError::Panicked(_)));
    assert!(err.to_string().contains("unwrap"), "{err}");
}

#[test]
fn a_missing_source_is_an_error_not_a_panic() {
    let missing = fixture().with_file_name("does-not-exist.ufo");

    let err = compile_to_ttf(&missing).unwrap_err();

    assert!(matches!(err, CompileError::Fontc(_)));
    assert!(err.to_string().contains("does-not-exist.ufo"));
}
