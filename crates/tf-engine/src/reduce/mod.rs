//! The dispatch from a catalog [`Command`] to its reducer. A reducer is a pure function
//! `(&DocState, &payload) -> Result<Reduced, CommandError>`: it never changes its input,
//! and an error leaves nothing behind.
//!
//! One file per command family, so Steps 5 and 6 work apart: `structure` and `font`
//! (Step 5), `outline` and `parts` (Step 6). An arm whose reducer is not written yet calls
//! a stub in its family file that fails with `E_NOT_IMPLEMENTED`. The document commands of
//! kind E and D (`doc.new`, `doc.open`, `doc.save`, `doc.saveAs`, `doc.close`) are engine
//! operations of Step 13; `doc.undo` and `doc.redo` run through
//! [`Document`](crate::Document)'s history, not through a reducer.

use std::collections::BTreeSet;

use tf_commands::{Command, CommandError, CommandId, ErrorCode};
use tf_model::{Font, GlyphId, LayerId, Name, SourceId};

use crate::document::DocState;

/// Declares reducers that a later step writes: each fails with `E_NOT_IMPLEMENTED`.
macro_rules! not_implemented {
    ($($reducer:ident($payload:ty) = $id:literal;)*) => {$(
        pub(super) fn $reducer(
            _: &$crate::document::DocState,
            _: &$payload,
        ) -> Result<super::Reduced, tf_commands::CommandError> {
            Err(tf_commands::CommandError::new(
                tf_commands::ErrorCode::NotImplemented,
                concat!($id, " is not implemented yet"),
            ))
        }
    )*};
}

mod font;
mod outline;
mod parts;
mod structure;

/// An entity a command changed. Patches (Step 6) and diffs (Step 7) are built from these.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Touched {
    /// A whole glyph: created, deleted, renamed, or a field of every layer.
    Glyph(GlyphId),
    /// One glyph layer (one `.glif`): width, outline, components, anchors.
    GlyphLayer(GlyphId, LayerId),
    /// One `fontinfo.plist` key of a source.
    FontInfo(SourceId, String),
    /// One kerning pair of a source.
    KerningPair(SourceId, Name, Name),
    /// One group of a source.
    Group(SourceId, Name),
    /// The `features.fea` text of a source.
    Features(SourceId),
    /// One `lib.plist` key of a source.
    SourceLib(SourceId, String),
}

/// Something a client should hear about a command, beyond what it touched.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// A glyph was created with this ID.
    GlyphCreated { id: GlyphId, name: Name },
    /// The command succeeded, but something needs the user's attention.
    Warning { message: String },
}

/// What a reducer returns: the next state, its events and the entities it changed. An
/// empty `touched` means nothing changed: the document records no step.
#[derive(Clone, Debug)]
pub struct Reduced {
    pub state: DocState,
    pub events: Vec<Event>,
    pub touched: BTreeSet<Touched>,
}

impl Reduced {
    /// `font` as the next revision after `prev`.
    pub(crate) fn new(prev: &DocState, font: Font, touched: BTreeSet<Touched>) -> Self {
        Self {
            state: DocState {
                glyph_slots: prev.glyph_slots.max(font.glyphs().slot_count()),
                font,
                revision: prev.revision.next(),
            },
            events: Vec::new(),
            touched,
        }
    }

    /// No change: `prev` again, nothing touched.
    pub(crate) fn unchanged(prev: &DocState) -> Self {
        Self {
            state: prev.clone(),
            events: Vec::new(),
            touched: BTreeSet::new(),
        }
    }
}

/// Runs the reducer of `command` on `state`.
pub fn reduce(state: &DocState, command: &Command) -> Result<Reduced, CommandError> {
    use Command as C;
    // No wildcard: a new catalog command must get an arm.
    match command {
        C::DocNew(_) | C::DocOpen(_) | C::DocSave(_) | C::DocSaveAs(_) | C::DocClose(_) => {
            Err(CommandError::new(
                ErrorCode::NotImplemented,
                format!("{} is not implemented yet", command.id().as_str()),
            ))
        }
        C::DocUndo(_) | C::DocRedo(_) => Err(history_only(command.id())),
        C::GlyphCreate(p) => structure::create(state, p),
        C::GlyphDelete(p) => structure::delete(state, p),
        C::GlyphRename(p) => structure::rename(state, p),
        C::GlyphDuplicate(p) => structure::duplicate(state, p),
        C::GlyphSetCodepoints(p) => structure::set_codepoints(state, p),
        C::GlyphSetWidth(p) => structure::set_width(state, p),
        C::GlyphSetOutline(p) => outline::set_outline(state, p),
        C::GlyphMovePoints(p) => outline::move_points(state, p),
        C::GlyphInsertPoint(p) => outline::insert_point(state, p),
        C::GlyphDeletePoints(p) => outline::delete_points(state, p),
        C::GlyphToggleSmooth(p) => outline::toggle_smooth(state, p),
        C::GlyphTransform(p) => outline::transform(state, p),
        C::GlyphAddComponent(p) => parts::add_component(state, p),
        C::GlyphRemoveComponent(p) => parts::remove_component(state, p),
        C::GlyphDecompose(p) => parts::decompose(state, p),
        C::GlyphAddAnchor(p) => parts::add_anchor(state, p),
        C::GlyphMoveAnchor(p) => parts::move_anchor(state, p),
        C::GlyphRemoveAnchor(p) => parts::remove_anchor(state, p),
        C::FontSetInfo(p) => font::set_info(state, p),
        C::FontSetVerticalMetrics(p) => font::set_vertical_metrics(state, p),
        C::KerningSetPairs(p) => font::set_kerning_pairs(state, p),
        C::KerningRemovePairs(p) => font::remove_kerning_pairs(state, p),
        C::KerningSetGroup(p) => font::set_kerning_group(state, p),
        C::FeaturesSetCode(p) => font::set_features(state, p),
    }
}

fn history_only(id: CommandId) -> CommandError {
    CommandError::new(
        ErrorCode::NotImplemented,
        format!(
            "{} has no reducer: run it through Document::execute",
            id.as_str()
        ),
    )
}

/// Generators of command sequences for the history property tests (one module per family).
#[cfg(test)]
pub(crate) mod testing {
    use proptest::prelude::*;
    use proptest::sample::Index;
    use tf_commands::Command;
    use tf_model::{Font, GlyphId, LayerId};

    /// Random picks that a family's `build` resolves against the font at run time, so the
    /// generated commands mostly target live glyphs and layers.
    #[derive(Clone, Debug)]
    pub struct Pick {
        pub a: Index,
        pub b: Index,
        pub value: i16,
    }

    /// One generated command: built from the font it will run on.
    #[derive(Clone, Debug)]
    pub struct CommandGen {
        pub pick: Pick,
        pub build: fn(&Font, &Pick) -> Command,
    }

    impl CommandGen {
        pub fn build(&self, font: &Font) -> Command {
            (self.build)(font, &self.pick)
        }
    }

    /// A live glyph picked by `index`, or glyph 0 in an empty font.
    pub fn live_glyph(font: &Font, index: &Index) -> GlyphId {
        let ids: Vec<GlyphId> = font.glyphs().iter().map(|(id, _)| id).collect();
        if ids.is_empty() {
            GlyphId(0)
        } else {
            *index.get(&ids)
        }
    }

    /// One of the glyph's layers picked by `index`, or layer 0 when it has none.
    pub fn live_layer(font: &Font, glyph: GlyphId, index: &Index) -> LayerId {
        let layers: Vec<LayerId> = font
            .glyph(glyph)
            .map(|g| g.layers.keys().copied().collect())
            .unwrap_or_default();
        if layers.is_empty() {
            LayerId(0)
        } else {
            *index.get(&layers)
        }
    }

    /// A strategy for one family's commands from its `build` functions.
    pub fn family(builds: &[fn(&Font, &Pick) -> Command]) -> BoxedStrategy<CommandGen> {
        let builds = builds.to_vec();
        (
            any::<Index>(),
            any::<Index>(),
            any::<i16>(),
            prop::sample::select(builds),
        )
            .prop_map(|(a, b, value, build)| CommandGen {
                pick: Pick { a, b, value },
                build,
            })
            .boxed()
    }

    /// Every family's commands: the history property tests draw from this one list. Each
    /// family step adds its line.
    pub fn command() -> impl Strategy<Value = CommandGen> {
        prop_oneof![
            super::structure::testing::commands(),
            // Step 5: super::font::testing::commands(),
            // Step 6: super::outline::testing::commands(), super::parts::testing::commands(),
        ]
    }
}

/// A small hand-made font for unit tests.
#[cfg(test)]
pub(crate) mod fixture {
    use std::sync::Arc;

    use tf_model::{
        Component, Font, Glyph, GlyphLayer, LayerId, Name, Source, SourceId, SourceLayer, Transform,
    };

    pub fn name(s: &str) -> Name {
        Name::new(s).unwrap()
    }

    /// Source 0 has layers 0 (default) and 1 (background); source 1 has layer 2. Glyphs:
    /// 0 `A` (layers 0 and 2, code point A), 1 `acute` (layer 0), 2 `Aacute` (layer 0:
    /// components `A` and `acute`; layer 2: component `A`), 3 `space` (layer 0).
    pub fn font() -> Font {
        let mut source = Source::new(SourceLayer::new(LayerId(0), name("public.default")));
        source
            .layers
            .push(SourceLayer::new(LayerId(1), name("public.background")));
        let mut font = Font::new(SourceId(0), source).unwrap();
        font.add_source(
            SourceId(1),
            Source::new(SourceLayer::new(LayerId(2), name("public.default"))),
        )
        .unwrap();
        let layer = |width: f64, codepoints: &[char], bases: &[&str]| {
            Arc::new(GlyphLayer {
                width,
                codepoints: codepoints.iter().copied().collect(),
                components: bases
                    .iter()
                    .map(|base| Component {
                        base: name(base),
                        transform: Transform::default(),
                        identifier: None,
                        lib: None,
                    })
                    .collect(),
                ..GlyphLayer::default()
            })
        };
        let glyph = |glyph_name: &str, layers: Vec<(u32, Arc<GlyphLayer>)>| {
            let mut glyph = Glyph::new(name(glyph_name));
            for (id, data) in layers {
                glyph.layers.insert(LayerId(id), data);
            }
            Arc::new(glyph)
        };
        for g in [
            glyph(
                "A",
                vec![
                    (0, layer(600.0, &['A'], &[])),
                    (2, layer(640.0, &['A'], &[])),
                ],
            ),
            glyph("acute", vec![(0, layer(300.0, &[], &[]))]),
            glyph(
                "Aacute",
                vec![
                    (0, layer(600.0, &['Á'], &["A", "acute"])),
                    (2, layer(640.0, &['Á'], &["A"])),
                ],
            ),
            glyph("space", vec![(0, layer(250.0, &[' '], &[]))]),
        ] {
            font.insert_glyph(g).unwrap();
        }
        font
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Document, ExecCtx, Origin};

    /// Every command of the catalog has an arm: the registry example of each one runs, and
    /// exactly the commands of this step and the history commands are implemented.
    #[test]
    fn every_catalog_command_is_dispatched() {
        // Each family file lists its own implemented commands.
        let implemented: Vec<CommandId> = [
            &[CommandId::DocUndo, CommandId::DocRedo][..],
            structure::IMPLEMENTED,
            font::IMPLEMENTED,
            outline::IMPLEMENTED,
            parts::IMPLEMENTED,
        ]
        .concat();
        for &id in CommandId::ALL {
            let example = tf_commands::registry::command(id).example;
            let command: Command = serde_json::from_str(example).unwrap();
            let mut document = Document::new(fixture::font());
            let result = document.execute(&ExecCtx::new(Origin::User, 0), &command);
            let not_implemented = matches!(
                &result,
                Err(e) if e.code == ErrorCode::NotImplemented
            );
            assert_eq!(
                not_implemented,
                !implemented.contains(&id),
                "{}: {result:?}",
                id.as_str()
            );
            if let Err(error) = result
                && not_implemented
            {
                assert!(error.message.starts_with(id.as_str()), "{error}");
            }
        }
    }

    #[test]
    fn history_commands_have_no_reducer() {
        let state = DocState::new(fixture::font(), crate::Revision(0));
        let undo = Command::DocUndo(tf_commands::command::DocUndo {});
        let error = reduce(&state, &undo).unwrap_err();
        assert_eq!(error.code, ErrorCode::NotImplemented);
        assert!(error.message.contains("Document::execute"));
    }
}
