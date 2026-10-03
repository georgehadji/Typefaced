//! Spike 1: which designspace 5 features does fontc 1.0.0 compile? Each case writes a
//! small designspace into a temporary directory, pointing at the two `WghtVar` UFOs of
//! the corpus, and pins what fontc makes of it. A changed outcome after a fontc upgrade
//! fails this test on purpose: update the table and the spike report together.

use std::path::Path;

use skrifa::raw::TableProvider;
use skrifa::{FontRef, MetadataProvider};
use tf_compile::compile_to_ttf;

/// The absolute path of a corpus UFO, with `/` separators, for a `filename` attribute.
fn ufo(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/.cache/fontc-testdata")
        .join(name);
    path.to_string_lossy().replace('\\', "/")
}

/// A source at `wght` (and at `ital`, for cases with the discrete italic axis).
fn source(name: &str, file: &str, wght: u32, ital: Option<u32>) -> String {
    let ital = ital
        .map(|v| format!(r#"<dimension name="Italic" xvalue="{v}"/>"#))
        .unwrap_or_default();
    format!(
        r#"<source filename="{}" name="{name}"><location><dimension name="Weight" xvalue="{wght}"/>{ital}</location></source>"#,
        ufo(file)
    )
}

const WEIGHT_AXIS: &str =
    r#"<axis tag="wght" name="Weight" minimum="400" maximum="700" default="400"/>"#;
const PANIC: &str = "called `Option::unwrap()` on a `None` value";

/// Name, document body (inside `<designspace format="5.0">`) and expected outcome.
fn cases() -> Vec<(&'static str, String, String)> {
    let sources = source("Regular", "WghtVar-Regular.ufo", 400, None)
        + &source("Bold", "WghtVar-Bold.ufo", 700, None);
    let italic_sources = source("Regular", "WghtVar-Regular.ufo", 400, Some(0))
        + &source("Bold", "WghtVar-Bold.ufo", 700, Some(0))
        + &source("Italic", "WghtVar-Regular.ufo", 400, Some(1))
        + &source("BoldItalic", "WghtVar-Bold.ufo", 700, Some(1));
    let ignored = "ok: axes=wght avar=none stat-values=0".to_owned();
    vec![
        (
            "format 5.0, continuous axis only",
            format!("<axes>{WEIGHT_AXIS}</axes><sources>{sources}</sources>"),
            ignored.clone(),
        ),
        (
            "discrete axis",
            format!(
                r#"<axes>{WEIGHT_AXIS}<axis tag="ital" name="Italic" values="0 1" default="0"/></axes><sources>{italic_sources}</sources>"#
            ),
            format!("error: fontc panicked: {PANIC}"),
        ),
        (
            "axis labels and elidedfallbackname (no STAT values made)",
            format!(
                r#"<axes elidedfallbackname="Regular"><axis tag="wght" name="Weight" minimum="400" maximum="700" default="400"><labels><label uservalue="400" name="Regular" elidable="true"/><label uservalue="700" name="Bold"/></labels></axis></axes><sources>{sources}</sources>"#
            ),
            ignored.clone(),
        ),
        (
            "avar2 axis mappings (no avar made)",
            format!(
                r#"<axes>{WEIGHT_AXIS}<mappings><mapping><input><dimension name="Weight" xvalue="550"/></input><output><dimension name="Weight" xvalue="600"/></output></mapping></mappings></axes><sources>{sources}</sources>"#
            ),
            ignored.clone(),
        ),
        (
            "location labels",
            format!(
                r#"<axes>{WEIGHT_AXIS}</axes><labels><label name="Semibold"><location><dimension name="Weight" uservalue="600"/></location></label></labels><sources>{sources}</sources>"#
            ),
            ignored.clone(),
        ),
        (
            "variable-fonts with two weight subsets (one font made, subsets ignored)",
            format!(
                r#"<axes>{WEIGHT_AXIS}</axes><sources>{sources}</sources><variable-fonts><variable-font name="Full"><axis-subsets><axis-subset name="Weight"/></axis-subsets></variable-font><variable-font name="Light"><axis-subsets><axis-subset name="Weight" userminimum="400" usermaximum="550"/></axis-subsets></variable-font></variable-fonts>"#
            ),
            ignored.clone(),
        ),
        (
            "instance located by user values",
            format!(
                r#"<axes>{WEIGHT_AXIS}</axes><sources>{sources}</sources><instances><instance familyname="Wght Var" stylename="Semibold"><location><dimension name="Weight" uservalue="600"/></location></instance></instances>"#
            ),
            format!("error: A task panicked: '{PANIC}'"),
        ),
    ]
}

/// What fontc made of a case: the axes, the avar version and the number of STAT axis
/// values, or the error text.
fn compile_case(dir: &Path, index: usize, body: &str) -> String {
    let path = dir.join(format!("case{index}.designspace"));
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><designspace format="5.0">{body}</designspace>"#
    );
    if let Err(e) = std::fs::write(&path, xml) {
        return format!("cannot write the case: {e}");
    }
    let bytes = match compile_to_ttf(&path) {
        Ok(bytes) => bytes,
        Err(e) => return format!("error: {e}"),
    };
    let font = match FontRef::new(&bytes) {
        Ok(font) => font,
        Err(e) => return format!("unreadable font: {e}"),
    };
    let axes: Vec<String> = font.axes().iter().map(|a| a.tag().to_string()).collect();
    let avar = font
        .avar()
        .map_or("none".to_owned(), |t| format!("v{}", t.version().major));
    let stat = font.stat().map_or(0, |t| t.axis_value_count());
    format!("ok: axes={} avar={avar} stat-values={stat}", axes.join(","))
}

#[test]
#[ignore = "needs corpus: cargo xtask corpus fetch"]
fn designspace_5_feature_support_is_as_recorded_in_the_spike_report() {
    let dir = std::env::temp_dir().join(format!("tf-compile-ds5-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let outcomes: Vec<String> = cases()
        .iter()
        .enumerate()
        .map(|(i, (name, body, expected))| {
            let found = compile_case(&dir, i, body);
            println!("{name}: {found}");
            format!(
                "{name}: {}",
                if &found == expected {
                    "as recorded"
                } else {
                    &found
                }
            )
        })
        .collect();
    std::fs::remove_dir_all(&dir).unwrap();

    let changed: Vec<&String> = outcomes
        .iter()
        .filter(|o| !o.ends_with("as recorded"))
        .collect();
    assert!(changed.is_empty(), "{changed:#?}");
}
