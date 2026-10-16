//! Proptest generators for model values, shared with other crates' property tests through the
//! `testing` feature (`tf-engine` builds command sequences on top of [`font`]).
//!
//! Every generated value is valid: generated fonts pass every `Font` check. They cover
//! several sources, background layers, glyphs missing from some layers (also the default
//! one), open and closed contours (rotated, so off-curves wrap around), components that
//! resolve and components that dangle, anchors, kerning, groups and a designspace.

use std::collections::BTreeSet;
use std::sync::Arc;

use proptest::prelude::*;

use crate::{
    Anchor, Component, Contour, Designspace, Font, Glyph, GlyphLayer, LayerId, Master, Name, Point,
    PointKind, Source, SourceId, SourceLayer, Transform,
};

/// The component base that never names a glyph of a generated font.
pub const DANGLING: &str = "dangling.base";

/// A glyph-like name.
pub fn name() -> impl Strategy<Value = Name> {
    "[A-Za-z][A-Za-z0-9._]{0,7}".prop_filter_map("valid name", |s| Name::new(&s).ok())
}

fn coordinate() -> impl Strategy<Value = f64> {
    (-1000i16..1000).prop_map(f64::from)
}

/// One segment: the off-curve count and the on-curve kind that ends it.
fn segment() -> impl Strategy<Value = (usize, PointKind)> {
    prop_oneof![
        Just((0, PointKind::Line)),
        (0usize..=2).prop_map(|offs| (offs, PointKind::Curve)),
        (0usize..=3).prop_map(|offs| (offs, PointKind::QCurve)),
    ]
}

/// A valid contour: open (starts with `move`), closed and rotated, or all off-curve.
pub fn contour() -> impl Strategy<Value = Contour> {
    let kinds = prop_oneof![
        3 => (prop::collection::vec(segment(), 1..6), any::<bool>(), any::<usize>()).prop_map(
            |(segments, open, rotation)| {
                let mut kinds: Vec<PointKind> = segments
                    .into_iter()
                    .flat_map(|(offs, kind)| {
                        std::iter::repeat_n(PointKind::OffCurve, offs).chain([kind])
                    })
                    .collect();
                if open {
                    kinds.insert(0, PointKind::Move);
                } else {
                    let len = kinds.len();
                    kinds.rotate_left(rotation % len);
                }
                kinds
            }
        ),
        1 => (1usize..5).prop_map(|n| vec![PointKind::OffCurve; n]),
    ];
    kinds.prop_flat_map(|kinds| {
        let n = kinds.len();
        (
            Just(kinds),
            prop::collection::vec((coordinate(), coordinate(), any::<bool>()), n),
        )
            .prop_map(|(kinds, coords)| {
                let points = kinds
                    .into_iter()
                    .zip(coords)
                    .map(|(kind, (x, y, smooth))| Point {
                        smooth: smooth && kind != PointKind::OffCurve,
                        ..Point::new(x, y, kind)
                    })
                    .collect();
                Contour::new(points)
            })
    })
}

/// A glyph layer whose components name glyphs from `bases` (which should include
/// [`DANGLING`] to cover unresolved references).
pub fn glyph_layer(bases: Vec<Name>) -> impl Strategy<Value = GlyphLayer> {
    let component = (prop::sample::select(bases), coordinate()).prop_map(|(base, dx)| Component {
        base,
        transform: Transform {
            x_offset: dx,
            ..Transform::default()
        },
        identifier: None,
        lib: None,
    });
    let anchor =
        (prop::option::of(name()), coordinate(), coordinate()).prop_map(|(name, x, y)| Anchor {
            x,
            y,
            name,
            color: None,
            identifier: None,
            lib: None,
        });
    (
        0u16..1200,
        prop::collection::btree_set(prop::char::range('A', 'z'), 0..3),
        prop::collection::vec(contour(), 0..3),
        prop::collection::vec(component, 0..2),
        prop::collection::vec(anchor, 0..2),
    )
        .prop_map(
            |(width, codepoints, contours, components, anchors)| GlyphLayer {
                width: f64::from(width),
                codepoints: codepoints.into_iter().collect(),
                contours,
                components,
                anchors,
                ..GlyphLayer::default()
            },
        )
}

/// Builds the sources: source `s` gets `layer_counts[s]` layers with consecutive IDs.
fn sources(layer_counts: &[usize]) -> Option<Vec<Source>> {
    let mut next = 0u32;
    let mut sources = Vec::new();
    for &count in layer_counts {
        let mut layers = Vec::new();
        for i in 0..count {
            let layer_name = if i == 0 {
                "public.default".to_owned()
            } else {
                format!("background{i}")
            };
            layers.push(SourceLayer::new(
                LayerId(next),
                Name::new(&layer_name).ok()?,
            ));
            next += 1;
        }
        let mut layers = layers.into_iter();
        let mut source = Source::new(layers.next()?);
        source.layers.extend(layers);
        sources.push(source);
    }
    Some(sources)
}

/// A valid font: 1–3 sources of 1–2 layers, up to 12 glyphs each present in a non-empty
/// subset of the layers, kerning and one group per source, and a designspace when there is
/// more than one source.
pub fn font() -> impl Strategy<Value = Font> {
    (
        prop::collection::vec(1usize..=2, 1..=3),
        prop::collection::btree_set(name(), 0..12),
    )
        .prop_flat_map(|(layer_counts, names)| {
            let names: Vec<Name> = names.into_iter().collect();
            let total_layers: usize = layer_counts.iter().sum();
            let mut bases = names.clone();
            bases.extend(Name::new(DANGLING).ok());
            let glyph = prop::collection::vec(
                prop::option::weighted(0.6, glyph_layer(bases)),
                total_layers,
            );
            let kerning = prop::collection::vec(
                (
                    any::<prop::sample::Index>(),
                    any::<prop::sample::Index>(),
                    -200i16..200,
                ),
                0..6,
            );
            (
                Just(layer_counts.clone()),
                Just(names.clone()),
                prop::collection::vec(glyph, names.len()),
                prop::collection::vec(kerning, layer_counts.len()),
            )
        })
        .prop_filter_map(
            "buildable font",
            |(layer_counts, names, layers, kerning)| {
                build_font(&layer_counts, &names, layers, &kerning)
            },
        )
}

type KerningPicks = Vec<(prop::sample::Index, prop::sample::Index, i16)>;

fn build_font(
    layer_counts: &[usize],
    names: &[Name],
    layers: Vec<Vec<Option<GlyphLayer>>>,
    kerning: &[KerningPicks],
) -> Option<Font> {
    let mut sources = sources(layer_counts)?;
    if !names.is_empty() {
        for (source, picks) in sources.iter_mut().zip(kerning) {
            for (a, b, value) in picks {
                let pair = (a.get(names).clone(), b.get(names).clone());
                source.kerning.insert(pair, f64::from(*value));
            }
            let members: BTreeSet<Name> =
                picks.iter().map(|(a, _, _)| a.get(names).clone()).collect();
            let group = Name::new("public.kern1.group").ok()?;
            source.groups.insert(group, members.into_iter().collect());
        }
    }
    let mut sources = sources.into_iter().enumerate();
    let (_, first) = sources.next()?;
    let mut font = Font::new(SourceId(0), first).ok()?;
    for (index, source) in sources {
        font.add_source(SourceId(u32::try_from(index).ok()?), source)
            .ok()?;
    }
    for (glyph_name, glyph_layers) in names.iter().zip(layers) {
        let mut glyph = Glyph::new(glyph_name.clone());
        for (layer, data) in glyph_layers.into_iter().enumerate() {
            if let Some(data) = data {
                glyph
                    .layers
                    .insert(LayerId(u32::try_from(layer).ok()?), Arc::new(data));
            }
        }
        if glyph.layers.is_empty() {
            glyph.layers.insert(LayerId(0), Arc::default());
        }
        font.insert_glyph(Arc::new(glyph)).ok()?;
    }
    if layer_counts.len() > 1 {
        let masters = font
            .sources()
            .map(|(id, _)| Master {
                name: Some(format!("master{}", id.0)),
                familyname: None,
                stylename: None,
                filename: format!("master{}.ufo", id.0),
                source: id,
                layer: None,
                location: Vec::new(),
            })
            .collect();
        font.set_designspace(Some(Designspace {
            format: 5.0,
            masters,
            ..Designspace::default()
        }))
        .ok()?;
    }
    Some(font)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::check_contour;

    proptest! {
        #[test]
        fn generated_contours_are_valid(contour in contour()) {
            prop_assert_eq!(check_contour(&contour.points), Ok(()));
        }

        /// A `move` anywhere but first breaks any generated contour.
        #[test]
        fn a_late_move_is_refused(contour in contour(), at in any::<prop::sample::Index>()) {
            let mut points = contour.points;
            let at = 1 + at.index(points.len());
            points.insert(at, Point::new(0.0, 0.0, PointKind::Move));
            prop_assert!(check_contour(&points).is_err());
        }

        #[test]
        fn generated_fonts_keep_every_invariant(font in font()) {
            let mut seen = HashSet::new();
            for (id, glyph) in font.glyphs().iter() {
                prop_assert!(seen.insert(glyph.name.clone()), "duplicate name {}", glyph.name);
                prop_assert_eq!(font.glyphs().id_of(&glyph.name), Some(id));
                prop_assert!(!glyph.layers.is_empty());
                for (layer, data) in &glyph.layers {
                    prop_assert!(font.layer_source(*layer).is_some());
                    for contour in &data.contours {
                        prop_assert_eq!(check_contour(&contour.points), Ok(()));
                    }
                }
            }
            prop_assert_eq!(seen.len(), font.glyphs().len());
            // Every warning is a dangling component to a name the font lacks.
            for warning in font.warnings() {
                let crate::Warning::DanglingComponent { base, .. } = warning;
                prop_assert!(font.glyphs().id_of(&base).is_none());
            }
            // Compacting a generated font changes nothing but IDs (there are no tombstones).
            prop_assert_eq!(font.compact(), font.clone());
        }
    }
}
