//! Cubic source outlines for the OTF transplant: every glyph of a UFO's default layer,
//! components decomposed through their transforms, keyed by the glyph name fontc
//! writes into the font.

use std::collections::HashMap;
use std::path::Path;

use kurbo::{Affine, BezPath};
use norad::{DataRequest, Font, Glyph, Layer, Plist};

use crate::CompileError;

/// Deeper nesting than this is treated as a component cycle.
const MAX_COMPONENT_DEPTH: usize = 64;

/// Decomposed cubic outlines by glyph name. Every component is replaced by its base
/// glyph's contours, transformed (contours reversed when the transform mirrors), so no
/// seac accents or components reach the CFF writer.
#[derive(Debug, Clone, Default)]
pub struct SourceOutlines {
    paths: HashMap<String, BezPath>,
}

impl SourceOutlines {
    /// Reads every glyph of the UFO's default layer. Glyphs are keyed by the names
    /// fontc 1.0.0 gives them with its default options: the `public.postscriptNames`
    /// production names (characters other than `A–Z a–z 0–9 . _` removed), unless the
    /// lib sets `com.github.googlei18n.ufo2ft.useProductionNames` to false.
    pub fn from_ufo(ufo: &Path) -> Result<Self, CompileError> {
        let request = DataRequest::none().default_layer(true).lib(true);
        let font = Font::load_requested_data(ufo, request)?;
        Self::from_font(&font)
    }

    fn from_font(font: &Font) -> Result<Self, CompileError> {
        let layer = font.default_layer();
        let rename = production_names(&font.lib);
        let mut paths = HashMap::new();
        for glyph in layer.iter() {
            let path = decompose(layer, glyph, 0)?;
            let name = final_name(glyph.name().as_str(), rename.as_ref());
            if paths.insert(name.clone(), path).is_some() {
                // fontc would add a `.N` suffix; this reader does not follow it.
                return Err(CompileError::DuplicateProductionName(name));
            }
        }
        Ok(Self { paths })
    }

    /// The outline of the glyph with this (production) name.
    pub fn get(&self, name: &str) -> Option<&BezPath> {
        self.paths.get(name)
    }

    /// Every glyph name and outline, in no particular order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &BezPath)> {
        self.paths.iter().map(|(name, path)| (name.as_str(), path))
    }
}

/// Outlines from another source than a UFO: (glyph name, decomposed cubic outline).
impl FromIterator<(String, BezPath)> for SourceOutlines {
    fn from_iter<I: IntoIterator<Item = (String, BezPath)>>(iter: I) -> Self {
        Self {
            paths: iter.into_iter().collect(),
        }
    }
}

/// The rename map fontc applies (ufo2fontir `postscript_names`, fontbe `post`).
fn production_names(lib: &Plist) -> Option<HashMap<String, String>> {
    let wanted = lib
        .get("com.github.googlei18n.ufo2ft.useProductionNames")
        .and_then(|v| v.as_boolean())
        .unwrap_or(true);
    let names = lib.get("public.postscriptNames")?.as_dictionary()?;
    wanted.then(|| {
        names
            .iter()
            .filter_map(|(glyph, ps)| Some((glyph.clone(), ps.as_string()?.to_owned())))
            .collect()
    })
}

fn final_name(name: &str, rename: Option<&HashMap<String, String>>) -> String {
    match rename {
        None => name.to_owned(),
        Some(rename) => {
            let mut name = rename.get(name).map_or(name, String::as_str).to_owned();
            name.retain(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_');
            name
        }
    }
}

/// The glyph's contours followed by its components' outlines, recursively.
fn decompose(layer: &Layer, glyph: &Glyph, depth: usize) -> Result<BezPath, CompileError> {
    let error = |reason: String| CompileError::Outline {
        glyph: glyph.name().to_string(),
        reason,
    };
    if depth > MAX_COMPONENT_DEPTH {
        return Err(error("components nest too deeply or form a cycle".into()));
    }
    let mut path = BezPath::new();
    for contour in &glyph.contours {
        if !contour.is_closed() {
            return Err(error("open contour: CFF closes every contour".into()));
        }
        path.extend(contour.to_kurbo().map_err(|e| error(e.to_string()))?);
    }
    for component in &glyph.components {
        let base_name = component.base.as_str();
        let base = layer
            .get_glyph(base_name)
            .ok_or_else(|| error(format!("component base {base_name:?} is missing")))?;
        let transform = Affine::from(component.transform);
        let outline = transform * decompose(layer, base, depth + 1)?;
        // A mirroring transform reverses the direction; flip it back (as fontc does).
        if transform.determinant() < 0.0 {
            path.extend(outline.reverse_subpaths());
        } else {
            path.extend(outline);
        }
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Point;
    use norad::{AffineTransform, Component, Contour, ContourPoint, Name, PointType};

    fn point(x: f64, y: f64, typ: PointType) -> ContourPoint {
        ContourPoint::new(x, y, typ, false, None, None)
    }

    /// A closed triangle, counter-clockwise.
    fn triangle() -> Contour {
        Contour::new(
            vec![
                point(0.0, 0.0, PointType::Line),
                point(100.0, 0.0, PointType::Line),
                point(50.0, 100.0, PointType::Line),
            ],
            None,
        )
    }

    fn glyph(name: &str, contours: Vec<Contour>, components: Vec<Component>) -> Glyph {
        let mut glyph = Glyph::new(name);
        glyph.contours = contours;
        glyph.components = components;
        glyph
    }

    fn component(base: &str, transform: AffineTransform) -> Component {
        Component::new(Name::new(base).unwrap(), transform, None)
    }

    fn font(glyphs: Vec<Glyph>) -> Font {
        let mut font = Font::new();
        for glyph in glyphs {
            font.default_layer_mut().insert_glyph(glyph);
        }
        font
    }

    fn translate(x: f64, y: f64) -> AffineTransform {
        AffineTransform {
            x_offset: x,
            y_offset: y,
            ..AffineTransform::default()
        }
    }

    fn signed_area(path: &BezPath) -> f64 {
        kurbo::Shape::area(path)
    }

    #[test]
    fn a_component_is_its_base_moved_by_the_offset() {
        let font = font(vec![
            glyph("base", vec![triangle()], vec![]),
            glyph(
                "composite",
                vec![],
                vec![component("base", translate(10.0, 20.0))],
            ),
        ]);

        let outlines = SourceOutlines::from_font(&font).unwrap();

        let base = outlines.get("base").unwrap();
        let composite = outlines.get("composite").unwrap();
        assert_eq!(Affine::translate((10.0, 20.0)) * base, *composite);
    }

    #[test]
    fn nested_components_are_decomposed_through_both_transforms() {
        let scale = AffineTransform {
            x_scale: 2.0,
            y_scale: 2.0,
            ..AffineTransform::default()
        };
        let font = font(vec![
            glyph("base", vec![triangle()], vec![]),
            glyph("middle", vec![], vec![component("base", scale)]),
            glyph(
                "top",
                vec![triangle()],
                vec![component("middle", translate(5.0, 0.0))],
            ),
        ]);

        let outlines = SourceOutlines::from_font(&font).unwrap();

        let top = outlines.get("top").unwrap();
        let points: Vec<Point> = top
            .elements()
            .iter()
            .filter_map(|el| el.end_point())
            .collect();
        // Own contour first, then the component: (50, 100) scaled by 2, moved by 5.
        assert_eq!(points.len(), 8);
        assert!(points.contains(&Point::new(105.0, 200.0)));
    }

    #[test]
    fn a_mirroring_component_keeps_the_contour_direction() {
        let mirror = AffineTransform {
            x_scale: -1.0,
            ..AffineTransform::default()
        };
        let font = font(vec![
            glyph("base", vec![triangle()], vec![]),
            glyph("mirrored", vec![], vec![component("base", mirror)]),
        ]);

        let outlines = SourceOutlines::from_font(&font).unwrap();

        // Mirroring alone flips the sign of the signed area; reversing flips it back.
        let base_area = signed_area(outlines.get("base").unwrap());
        let mirrored_area = signed_area(outlines.get("mirrored").unwrap());
        assert_ne!(base_area, 0.0);
        assert_eq!(mirrored_area, base_area);
    }

    #[test]
    fn production_names_are_applied_and_cleaned_like_fontc() {
        let mut font = font(vec![
            glyph("Delta", vec![triangle()], vec![]),
            glyph("a-b", vec![], vec![]),
            glyph("kept", vec![], vec![]),
        ]);
        let mut names = Plist::new();
        names.insert("Delta".into(), "uni0394".into());
        names.insert("a-b".into(), "a-b".into());
        font.lib
            .insert("public.postscriptNames".into(), names.into());

        let outlines = SourceOutlines::from_font(&font).unwrap();

        let mut keys: Vec<&str> = outlines.iter().map(|(name, _)| name).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["ab", "kept", "uni0394"]);
    }

    #[test]
    fn production_names_can_be_turned_off() {
        let mut font = font(vec![glyph("Delta", vec![], vec![])]);
        let mut names = Plist::new();
        names.insert("Delta".into(), "uni0394".into());
        font.lib
            .insert("public.postscriptNames".into(), names.into());
        font.lib.insert(
            "com.github.googlei18n.ufo2ft.useProductionNames".into(),
            false.into(),
        );

        let outlines = SourceOutlines::from_font(&font).unwrap();

        assert!(outlines.get("Delta").is_some());
    }

    #[test]
    fn two_glyphs_with_one_production_name_are_an_error() {
        let mut font = font(vec![glyph("a", vec![], vec![]), glyph("b", vec![], vec![])]);
        let mut names = Plist::new();
        names.insert("a".into(), "same".into());
        names.insert("b".into(), "same".into());
        font.lib
            .insert("public.postscriptNames".into(), names.into());

        let err = SourceOutlines::from_font(&font).unwrap_err();

        assert!(matches!(err, CompileError::DuplicateProductionName(ref n) if n == "same"));
    }

    fn outline_error(glyphs: Vec<Glyph>) -> String {
        match SourceOutlines::from_font(&font(glyphs)) {
            Err(err @ CompileError::Outline { .. }) => err.to_string(),
            other => format!("expected an outline error, got {other:?}"),
        }
    }

    #[test]
    fn a_missing_component_base_is_an_error() {
        let message = outline_error(vec![glyph(
            "a",
            vec![],
            vec![component("nowhere", translate(0.0, 0.0))],
        )]);

        assert!(message.contains("\"nowhere\" is missing"), "{message}");
    }

    #[test]
    fn a_component_cycle_is_an_error() {
        let message = outline_error(vec![
            glyph("a", vec![], vec![component("b", translate(0.0, 0.0))]),
            glyph("b", vec![], vec![component("a", translate(0.0, 0.0))]),
        ]);

        assert!(message.contains("cycle"), "{message}");
    }

    #[test]
    fn an_open_contour_is_an_error() {
        let open = Contour::new(
            vec![
                point(0.0, 0.0, PointType::Move),
                point(10.0, 0.0, PointType::Line),
            ],
            None,
        );

        let message = outline_error(vec![glyph("a", vec![open], vec![])]);

        assert!(message.contains("open contour"), "{message}");
    }

    #[test]
    fn a_contour_norad_cannot_convert_is_an_error() {
        let bad = Contour::new(vec![point(0.0, 0.0, PointType::Curve)], None);

        let message = outline_error(vec![glyph("a", vec![bad], vec![])]);

        assert!(message.starts_with("glyph \"a\""), "{message}");
    }
}
