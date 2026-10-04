//! Corpus-backed checks for Spike 1 (ADR-0006). They need the pinned corpus:
//! `cargo xtask corpus fetch`, then `cargo test -p tf-compile -- --ignored`.

use std::path::{Path, PathBuf};

use skrifa::raw::TableProvider;
use skrifa::{FontRef, MetadataProvider, Tag};
use tf_compile::{CompileError, compile_to_ttf};

fn corpus(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/.cache")
        .join(relative)
}

/// The number of `.glif` files in a UFO's default layer.
fn source_glyph_count(ufo: &Path) -> std::io::Result<usize> {
    Ok(std::fs::read_dir(ufo.join("glyphs"))?
        .filter(|entry| {
            entry
                .as_ref()
                .is_ok_and(|e| e.path().extension().is_some_and(|ext| ext == "glif"))
        })
        .count())
}

#[test]
#[ignore = "needs corpus: cargo xtask corpus fetch"]
fn every_style_of_the_static_family_compiles() {
    let styles = [
        "Regular",
        "Italic",
        "Light",
        "LightItalic",
        "Bold",
        "BoldItalic",
    ];
    for style in styles {
        let ufo = corpus(&format!("inria-sans/InriaSans-{style}.ufo"));

        let bytes = compile_to_ttf(&ufo).unwrap_or_else(|e| panic!("{style}: {e}"));
        let font = FontRef::new(&bytes).unwrap();

        let glyphs = usize::from(font.maxp().unwrap().num_glyphs());
        assert_eq!(glyphs, source_glyph_count(&ufo).unwrap(), "{style}");
        assert!(font.table_data(Tag::new(b"glyf")).is_some(), "{style}");
        assert!(font.charmap().map('a').is_some(), "{style}");
    }
}

#[test]
#[ignore = "needs corpus: cargo xtask corpus fetch"]
fn the_variable_family_fails_on_its_per_master_feature_files() {
    let designspace = corpus("source-sans-upright/SourceSans3VF-Upright.designspace");

    let err = compile_to_ttf(&designspace).unwrap_err();

    assert!(matches!(err, CompileError::Fontc(_)), "{err}");
    assert!(format!("{err:?}").contains("NonIdenticalFea"), "{err:?}");
}

/// Copies `from` into `to`, leaving out every `features.fea`.
fn copy_without_features(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_without_features(&entry.path(), &target)?;
        } else if entry.file_name() != "features.fea" {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

#[test]
#[ignore = "needs corpus: cargo xtask corpus fetch"]
fn the_variable_family_compiles_with_a_weight_axis_without_its_feature_files() {
    let family = corpus("source-sans-upright");
    let dir = std::env::temp_dir().join(format!("tf-compile-vf-{}", std::process::id()));
    copy_without_features(&family.join("Poles"), &dir.join("Poles")).unwrap();
    let designspace = dir.join("SourceSans3VF-Upright.designspace");
    std::fs::copy(
        family.join("SourceSans3VF-Upright.designspace"),
        &designspace,
    )
    .unwrap();

    let compiled = compile_to_ttf(&designspace);
    std::fs::remove_dir_all(&dir).unwrap();
    let bytes = compiled.unwrap();
    let font = FontRef::new(&bytes).unwrap();

    let axes: Vec<Tag> = font.axes().iter().map(|axis| axis.tag()).collect();
    assert_eq!(axes, [Tag::new(b"wght")]);
    assert!(font.table_data(Tag::new(b"gvar")).is_some());
    assert!(font.maxp().unwrap().num_glyphs() > 2000);
}
