//! Reducers of the glyph-structure family. Step 4 writes `glyph.create`, `glyph.delete`
//! and `glyph.setWidth`; Step 5 writes `glyph.rename`, `glyph.duplicate` and
//! `glyph.setCodepoints` (stubs until then).

use std::collections::BTreeSet;
use std::sync::Arc;
use tf_commands::command::{
    GlyphCreate, GlyphDelete, GlyphDuplicate, GlyphRename, GlyphSetCodepoints, GlyphSetWidth,
};

use tf_commands::{CommandError, ErrorCode};
use tf_model::{Font, Glyph, GlyphId, GlyphLayer, Name};

use super::{Event, Reduced, Touched};
use crate::document::DocState;
use crate::map;

not_implemented! {
    rename(GlyphRename) = "glyph.rename";
    duplicate(GlyphDuplicate) = "glyph.duplicate";
    set_codepoints(GlyphSetCodepoints) = "glyph.setCodepoints";
}

/// The commands of this family that have a reducer (checked by the dispatch test).
#[cfg(test)]
pub(super) const IMPLEMENTED: &[tf_commands::CommandId] = &[
    tf_commands::CommandId::GlyphCreate,
    tf_commands::CommandId::GlyphDelete,
    tf_commands::CommandId::GlyphSetWidth,
];

/// `glyph.create`: a glyph with the name, code points and width in the default layer of
/// every source (for the default source, the font's default layer).
pub(super) fn create(state: &DocState, p: &GlyphCreate) -> Result<Reduced, CommandError> {
    let name = map::name(&p.name)?;
    let width = map::units("width", p.width)?;
    let codepoints = p
        .codepoints
        .iter()
        .map(|&c| map::codepoint(c))
        .collect::<Result<Vec<char>, _>>()?;
    let font = &state.font;
    for &c in &codepoints {
        if let Some(owner) = codepoint_owner(font, c) {
            return Err(CommandError::new(
                ErrorCode::CodepointInUse,
                format!("U+{:04X} belongs to {owner}", u32::from(c)),
            ));
        }
    }
    let layer = Arc::new(GlyphLayer {
        width,
        codepoints: codepoints.into_iter().collect(),
        ..GlyphLayer::default()
    });
    let mut glyph = Glyph::new(name.clone());
    for (id, source) in font.sources() {
        // A source always has a layer; its first is its default. The font's default layer
        // (where code points are read) may be another layer of the default source.
        let default = if id == font.default_source() {
            Some(font.default_layer())
        } else {
            source.layers.first().map(|l| l.id)
        };
        if let Some(default) = default {
            glyph.layers.insert(default, Arc::clone(&layer));
        }
    }
    let mut next = font.clone();
    next.reserve_glyph_slots(state.glyph_slots)
        .map_err(map::model_error)?;
    let id = next
        .insert_glyph(Arc::new(glyph))
        .map_err(map::model_error)?;
    let mut reduced = Reduced::new(state, next, BTreeSet::from([Touched::Glyph(id)]));
    reduced.events.push(Event::GlyphCreated { id, name });
    Ok(reduced)
}

/// The glyph that has `c` in any of its layers.
fn codepoint_owner(font: &Font, c: char) -> Option<&Name> {
    // ponytail: a scan of every glyph layer; a code-point index in `tf-model` if create or
    // setCodepoints ever shows up in a profile.
    font.glyphs()
        .iter()
        .find(|(_, glyph)| glyph.layers.values().any(|l| l.codepoints.contains(&c)))
        .map(|(_, glyph)| &glyph.name)
}

/// The most component uses an `E_GLYPH_IN_USE` message names.
const MAX_LISTED_USES: usize = 10;

/// `glyph.delete`: deletes glyphs; refuses one that a glyph outside the set uses as a
/// component (`E_GLYPH_IN_USE`, naming every such use).
pub(super) fn delete(state: &DocState, p: &GlyphDelete) -> Result<Reduced, CommandError> {
    let font = &state.font;
    let ids: BTreeSet<GlyphId> = p.ids.iter().map(|&id| map::glyph_id(id)).collect();
    let mut names = BTreeSet::new();
    for &id in &ids {
        let glyph = font.glyph(id).ok_or_else(|| {
            CommandError::new(ErrorCode::GlyphNotFound, format!("no glyph {}", id.0))
        })?;
        names.insert(&glyph.name);
    }
    let uses: BTreeSet<(&Name, &Name)> = font
        .glyphs()
        .iter()
        .filter(|(id, _)| !ids.contains(id))
        .flat_map(|(_, user)| {
            user.layers
                .values()
                .flat_map(|layer| &layer.components)
                .filter(|component| names.contains(&component.base))
                .map(move |component| (&user.name, &component.base))
        })
        .collect();
    if !uses.is_empty() {
        let mut list: Vec<String> = uses
            .iter()
            .take(MAX_LISTED_USES)
            .map(|(user, base)| format!("{user} uses {base}"))
            .collect();
        if uses.len() > MAX_LISTED_USES {
            list.push(format!("and {} more", uses.len() - MAX_LISTED_USES));
        }
        return Err(CommandError::new(
            ErrorCode::GlyphInUse,
            format!("glyphs used as components: {}", list.join(", ")),
        ));
    }
    if ids.is_empty() {
        return Ok(Reduced::unchanged(state));
    }
    // `next` is a copy (a few pointers), dropped on error: the removals land together.
    let mut next = font.clone();
    ids.iter()
        .try_for_each(|&id| next.remove_glyph(id).map(drop))
        .map_err(map::model_error)?;
    Ok(Reduced::new(
        state,
        next,
        ids.into_iter().map(Touched::Glyph).collect(),
    ))
}

/// `glyph.setWidth`: the advance width of one glyph layer.
pub(super) fn set_width(state: &DocState, p: &GlyphSetWidth) -> Result<Reduced, CommandError> {
    let width = map::units("width", p.width)?;
    let (id, layer) = (map::glyph_id(p.id), map::layer_id(p.layer));
    let font = &state.font;
    let glyph = font
        .glyph(id)
        .ok_or_else(|| CommandError::new(ErrorCode::GlyphNotFound, format!("no glyph {}", id.0)))?;
    let data = glyph.layers.get(&layer).ok_or_else(|| {
        CommandError::new(
            ErrorCode::LayerNotFound,
            format!("glyph {} has no layer {}", glyph.name, layer.0),
        )
    })?;
    if data.width == width {
        return Ok(Reduced::unchanged(state));
    }
    let mut edited = (**glyph).clone();
    edited.layers.insert(
        layer,
        Arc::new(GlyphLayer {
            width,
            ..(**data).clone()
        }),
    );
    let mut next = font.clone();
    next.replace_glyph(id, Arc::new(edited))
        .map_err(map::model_error)?;
    Ok(Reduced::new(
        state,
        next,
        BTreeSet::from([Touched::GlyphLayer(id, layer)]),
    ))
}

/// Generators of this family's commands for the history property tests.
#[cfg(test)]
pub(crate) mod testing {
    use proptest::strategy::BoxedStrategy;
    use tf_commands::Command;
    use tf_commands::command::{self as dto, GlyphCreate, GlyphDelete, GlyphSetWidth};
    use tf_model::Font;

    use crate::reduce::testing::{CommandGen, Pick, family, live_glyph, live_layer};

    pub fn commands() -> BoxedStrategy<CommandGen> {
        family(&[create, delete, set_width, set_first_width])
    }

    /// A fresh name (longer than any generated one); a private-use code point every third
    /// time.
    fn create(font: &Font, pick: &Pick) -> Command {
        let slot = u32::try_from(font.glyphs().slot_count()).unwrap_or(u32::MAX);
        Command::GlyphCreate(GlyphCreate {
            name: format!("new.glyph.{slot}"),
            codepoints: if pick.value % 3 == 0 {
                vec![0xE000 + slot % 0x1000]
            } else {
                Vec::new()
            },
            width: f64::from(pick.value),
        })
    }

    /// Deletes one live glyph; fails when another glyph uses it as a component.
    fn delete(font: &Font, pick: &Pick) -> Command {
        Command::GlyphDelete(GlyphDelete {
            ids: vec![dto::GlyphId(live_glyph(font, &pick.a).0)],
        })
    }

    /// Sets the width of one layer of a live glyph.
    fn set_width(font: &Font, pick: &Pick) -> Command {
        let id = live_glyph(font, &pick.a);
        Command::GlyphSetWidth(GlyphSetWidth {
            id: dto::GlyphId(id.0),
            layer: dto::LayerId(live_layer(font, id, &pick.b).0),
            width: f64::from(pick.value),
        })
    }

    /// Sets the width of the first layer of the first live glyph: repeated, it exercises
    /// coalescing (same key).
    fn set_first_width(font: &Font, pick: &Pick) -> Command {
        let (id, glyph) = font
            .glyphs()
            .iter()
            .next()
            .map_or((0, None), |(id, g)| (id.0, Some(g)));
        let layer = glyph
            .and_then(|g| g.layers.keys().next())
            .map_or(0, |l| l.0);
        Command::GlyphSetWidth(GlyphSetWidth {
            id: dto::GlyphId(id),
            layer: dto::LayerId(layer),
            width: f64::from(pick.value),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use tf_commands::command::{GlyphId, LayerId};
    use tf_model::{GlyphId as Gid, LayerId as Lid};

    use super::*;
    use crate::Revision;
    use crate::reduce::fixture::{self, name};
    use crate::reduce::{Event, Touched};

    fn state() -> DocState {
        DocState::new(fixture::font(), Revision(7))
    }

    fn create_cmd(glyph: &str, codepoints: &[u32], width: f64) -> GlyphCreate {
        GlyphCreate {
            name: glyph.into(),
            codepoints: codepoints.to_vec(),
            width,
        }
    }

    fn code(result: Result<Reduced, CommandError>) -> ErrorCode {
        result.unwrap_err().code
    }

    #[test]
    fn create_uses_the_font_default_layer_in_the_default_source() {
        let mut font = fixture::font();
        font.set_default_layer(Lid(1)).unwrap();
        let state = DocState::new(font, Revision(0));
        let reduced = create(&state, &create_cmd("B", &[0x42], 0.0)).unwrap();
        let font = &reduced.state.font;
        let id = font.glyphs().id_of("B").unwrap();
        let layers: Vec<Lid> = font.glyph(id).unwrap().layers.keys().copied().collect();
        assert_eq!(layers, [Lid(1), Lid(2)]);
        assert_eq!(font.codepoints(id), ['B']);
    }

    #[test]
    fn create_skips_ids_that_an_earlier_state_handed_out() {
        let mut state = state();
        state.glyph_slots = 9;
        let reduced = create(&state, &create_cmd("B", &[], 0.0)).unwrap();
        assert_eq!(reduced.state.font.glyphs().id_of("B"), Some(Gid(9)));
        assert_eq!(reduced.state.glyph_slots, 10);
    }

    #[test]
    fn delete_names_at_most_ten_uses() {
        let mut font = fixture::font();
        let template = (**font.glyph(Gid(2)).unwrap()).clone();
        for i in 0..12 {
            let mut user = template.clone();
            user.name = name(&format!("user{i:02}"));
            for layer in user.layers.values_mut() {
                std::sync::Arc::make_mut(layer).codepoints.clear();
            }
            font.insert_glyph(std::sync::Arc::new(user)).unwrap();
        }
        let state = DocState::new(font, Revision(0));
        let p = GlyphDelete {
            ids: vec![GlyphId(1)],
        };
        let error = delete(&state, &p).unwrap_err();
        assert!(
            error.message.ends_with("user08 uses acute, and 3 more"),
            "{}",
            error.message
        );
    }

    #[test]
    fn create_adds_the_glyph_to_the_default_layer_of_every_source() {
        let state = state();
        let reduced = create(&state, &create_cmd("B", &[0x42, 0x62], 550.0)).unwrap();
        let font = &reduced.state.font;
        let id = font.glyphs().id_of("B").unwrap();
        assert_eq!(id, Gid(4));
        let glyph = font.glyph(id).unwrap();
        assert_eq!(
            glyph.layers.keys().copied().collect::<Vec<_>>(),
            [Lid(0), Lid(2)]
        );
        for layer in glyph.layers.values() {
            assert_eq!(layer.width, 550.0);
            assert_eq!(layer.codepoints.as_slice(), ['B', 'b']);
        }
        assert_eq!(reduced.state.revision, Revision(8));
        assert_eq!(reduced.touched, BTreeSet::from([Touched::Glyph(id)]));
        assert_eq!(
            reduced.events,
            [Event::GlyphCreated {
                id,
                name: name("B")
            }]
        );
        // The input is untouched.
        assert!(state.font.glyphs().id_of("B").is_none());
    }

    #[test]
    fn create_refuses_bad_names_widths_and_code_points() {
        let state = state();
        assert_eq!(
            code(create(&state, &create_cmd("A", &[], 0.0))),
            ErrorCode::NameInUse
        );
        assert_eq!(
            code(create(&state, &create_cmd("", &[], 0.0))),
            ErrorCode::InvalidName
        );
        assert_eq!(
            code(create(&state, &create_cmd("B", &[], f64::NAN))),
            ErrorCode::InvalidField
        );
        assert_eq!(
            code(create(&state, &create_cmd("B", &[0xD800], 0.0))),
            ErrorCode::InvalidField
        );
        assert_eq!(
            code(create(&state, &create_cmd("B", &[0x42, 0x42], 0.0))),
            ErrorCode::InvalidField
        );
        // `A` has U+0041; `Aacute` has U+00C1 (in layers 0 and 2).
        let in_use = create(&state, &create_cmd("B", &[0x42, 0xC1], 0.0)).unwrap_err();
        assert_eq!(in_use.code, ErrorCode::CodepointInUse);
        assert!(in_use.message.contains("U+00C1") && in_use.message.contains("Aacute"));
    }

    #[test]
    fn delete_leaves_tombstones() {
        let state = state();
        let p = GlyphDelete {
            ids: vec![GlyphId(3), GlyphId(3)],
        };
        let reduced = delete(&state, &p).unwrap();
        assert!(reduced.state.font.glyph(Gid(3)).is_none());
        assert_eq!(reduced.state.font.glyphs().slot_count(), 4);
        assert_eq!(reduced.touched, BTreeSet::from([Touched::Glyph(Gid(3))]));
        assert_eq!(reduced.state.revision, Revision(8));

        // A base glyph goes together with every glyph that uses it.
        let p = GlyphDelete {
            ids: vec![GlyphId(2), GlyphId(0), GlyphId(1)],
        };
        let reduced = delete(&state, &p).unwrap();
        assert_eq!(reduced.state.font.glyphs().len(), 1);
        assert_eq!(reduced.touched.len(), 3);

        // Nothing to delete: nothing changes.
        let reduced = delete(&state, &GlyphDelete { ids: vec![] }).unwrap();
        assert!(reduced.touched.is_empty());
        assert_eq!(reduced.state.revision, Revision(7));
    }

    #[test]
    fn delete_refuses_unknown_glyphs_and_glyphs_in_use() {
        let state = state();
        let unknown = GlyphDelete {
            ids: vec![GlyphId(3), GlyphId(9)],
        };
        assert_eq!(code(delete(&state, &unknown)), ErrorCode::GlyphNotFound);

        let in_use = delete(
            &state,
            &GlyphDelete {
                ids: vec![GlyphId(1), GlyphId(0)],
            },
        )
        .unwrap_err();
        assert_eq!(in_use.code, ErrorCode::GlyphInUse);
        // Every use, in name order, once.
        assert!(
            in_use.message.ends_with("Aacute uses A, Aacute uses acute"),
            "{}",
            in_use.message
        );
    }

    #[test]
    fn set_width_changes_one_layer() {
        let state = state();
        let p = GlyphSetWidth {
            id: GlyphId(0),
            layer: LayerId(2),
            width: 700.0,
        };
        let reduced = set_width(&state, &p).unwrap();
        let glyph = reduced.state.font.glyph(Gid(0)).unwrap();
        assert_eq!(glyph.layers[&Lid(2)].width, 700.0);
        assert_eq!(glyph.layers[&Lid(0)].width, 600.0);
        assert_eq!(
            reduced.touched,
            BTreeSet::from([Touched::GlyphLayer(Gid(0), Lid(2))])
        );

        // The same width: nothing changes.
        let same = GlyphSetWidth { width: 640.0, ..p };
        assert!(set_width(&state, &same).unwrap().touched.is_empty());
    }

    #[test]
    fn set_width_refuses_unknown_targets_and_bad_widths() {
        let state = state();
        let p = |id, layer, width| GlyphSetWidth {
            id: GlyphId(id),
            layer: LayerId(layer),
            width,
        };
        assert_eq!(
            code(set_width(&state, &p(9, 0, 1.0))),
            ErrorCode::GlyphNotFound
        );
        // Layer 1 exists, but `A` has no glyph there; layer 5 does not exist.
        assert_eq!(
            code(set_width(&state, &p(0, 1, 1.0))),
            ErrorCode::LayerNotFound
        );
        assert_eq!(
            code(set_width(&state, &p(0, 5, 1.0))),
            ErrorCode::LayerNotFound
        );
        assert_eq!(
            code(set_width(&state, &p(0, 0, f64::INFINITY))),
            ErrorCode::InvalidField
        );
    }
}
