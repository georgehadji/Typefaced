//! The aggregate root: a font project. Every change goes through a `Font` method that checks
//! the invariants first and leaves the font unchanged when it fails.
//!
//! Invariants: glyph names are unique; every contour is a valid UFO point sequence; every
//! `LayerId` a glyph or designspace master uses belongs to a source; layer IDs are unique
//! across sources and layer names within a source; no lib holds `public.objectLibs`.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use imbl::OrdMap;

use crate::validate::{Warning, check_glyph_layer, check_lib};
use crate::{Designspace, Glyph, GlyphId, GlyphTable, LayerId, ModelError, Source, SourceId, keys};

/// Project files that are not font sources (filled by `tf-io`'s `.typefaced` package, Step 11).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProjectFiles {
    /// The package's project id (a UUID string).
    pub id: Option<String>,
    /// Package `assets/` and `ai/` files and a folder project's `typefaced/` folder:
    /// path relative to the project root, `/`-separated → bytes.
    pub files: BTreeMap<String, Arc<[u8]>>,
}

/// A font project: sources (UFOs), glyphs across their layers, an optional designspace and
/// project files. Cloning copies a few pointers.
#[derive(Clone, Debug, PartialEq)]
pub struct Font {
    glyphs: GlyphTable,
    sources: OrdMap<SourceId, Arc<Source>>,
    default_source: SourceId,
    default_layer: LayerId,
    designspace: Option<Arc<Designspace>>,
    project: Arc<ProjectFiles>,
}

impl Font {
    /// A font with one source and no glyphs. The source's first layer is the default layer.
    pub fn new(id: SourceId, source: Source) -> Result<Self, ModelError> {
        check_source(id, &source, &BTreeSet::new())?;
        let default_layer = source.layers.first().ok_or(ModelError::NoLayers(id))?.id;
        Ok(Self {
            glyphs: GlyphTable::new(),
            sources: OrdMap::unit(id, Arc::new(source)),
            default_source: id,
            default_layer,
            designspace: None,
            project: Arc::default(),
        })
    }

    pub fn glyphs(&self) -> &GlyphTable {
        &self.glyphs
    }

    pub fn glyph(&self, id: GlyphId) -> Option<&Arc<Glyph>> {
        self.glyphs.get(id)
    }

    /// Sources in ID order.
    pub fn sources(&self) -> impl Iterator<Item = (SourceId, &Arc<Source>)> {
        self.sources.iter().map(|(id, source)| (*id, source))
    }

    pub fn source(&self, id: SourceId) -> Option<&Arc<Source>> {
        self.sources.get(&id)
    }

    /// The source of the default master; font-data commands use it when none is named.
    pub fn default_source(&self) -> SourceId {
        self.default_source
    }

    /// The default master's layer; a glyph's code points are read from it.
    pub fn default_layer(&self) -> LayerId {
        self.default_layer
    }

    /// The source a layer belongs to.
    pub fn layer_source(&self, layer: LayerId) -> Option<SourceId> {
        self.sources
            .iter()
            .find(|(_, source)| source.layer(layer).is_some())
            .map(|(id, _)| *id)
    }

    pub fn designspace(&self) -> Option<&Arc<Designspace>> {
        self.designspace.as_ref()
    }

    pub fn project(&self) -> &Arc<ProjectFiles> {
        &self.project
    }

    /// A glyph's code points: its default-layer code points; empty when the glyph is not in
    /// the default layer or does not exist.
    pub fn codepoints(&self, id: GlyphId) -> &[char] {
        self.glyphs
            .get(id)
            .map_or(&[], |glyph| glyph.codepoints(self.default_layer))
    }

    pub fn add_source(&mut self, id: SourceId, source: Source) -> Result<(), ModelError> {
        if self.sources.contains_key(&id) {
            return Err(ModelError::SourceExists(id));
        }
        check_source(id, &source, &self.layer_ids_except(None))?;
        self.sources.insert(id, Arc::new(source));
        Ok(())
    }

    /// Replaces a source and returns the old one. A layer that disappears must not be the
    /// default layer or be used by a glyph or a designspace master.
    pub fn replace_source(
        &mut self,
        id: SourceId,
        source: Arc<Source>,
    ) -> Result<Arc<Source>, ModelError> {
        let old = Arc::clone(
            self.sources
                .get(&id)
                .ok_or(ModelError::SourceNotFound(id))?,
        );
        check_source(id, &source, &self.layer_ids_except(Some(id)))?;
        for layer in old.layers.iter().map(|l| l.id) {
            if source.layer(layer).is_none() && self.layer_in_use(layer) {
                return Err(ModelError::LayerInUse(layer));
            }
        }
        self.sources.insert(id, source);
        Ok(old)
    }

    /// Makes `layer` the default layer, and its source the default source.
    pub fn set_default_layer(&mut self, layer: LayerId) -> Result<(), ModelError> {
        self.default_source = self
            .layer_source(layer)
            .ok_or(ModelError::LayerNotFound(layer))?;
        self.default_layer = layer;
        Ok(())
    }

    /// Sets or clears the designspace. Every master must name an existing source and, if it
    /// names a layer, a layer of that source.
    pub fn set_designspace(&mut self, designspace: Option<Designspace>) -> Result<(), ModelError> {
        for master in designspace.iter().flat_map(|d| &d.masters) {
            let source = self
                .sources
                .get(&master.source)
                .ok_or(ModelError::SourceNotFound(master.source))?;
            if let Some(layer) = master.layer
                && source.layer(layer).is_none()
            {
                return Err(ModelError::LayerNotFound(layer));
            }
        }
        self.designspace = designspace.map(Arc::new);
        Ok(())
    }

    pub fn set_project(&mut self, project: ProjectFiles) {
        self.project = Arc::new(project);
    }

    pub fn insert_glyph(&mut self, glyph: Arc<Glyph>) -> Result<GlyphId, ModelError> {
        self.check_glyph(&glyph)?;
        self.glyphs.insert(glyph)
    }

    /// Replaces a glyph (a new name renames it) and returns the old one.
    pub fn replace_glyph(
        &mut self,
        id: GlyphId,
        glyph: Arc<Glyph>,
    ) -> Result<Arc<Glyph>, ModelError> {
        self.check_glyph(&glyph)?;
        self.glyphs.replace(id, glyph)
    }

    /// Deletes a glyph, leaving a tombstone, and returns it.
    pub fn remove_glyph(&mut self, id: GlyphId) -> Result<Arc<Glyph>, ModelError> {
        self.glyphs.remove(id)
    }

    /// The same font with glyph IDs renumbered without tombstones. **Load only**: open
    /// sandboxes, undo snapshots and clients hold glyph IDs. Model data names glyphs, never
    /// IDs, so nothing else changes.
    pub fn compact(&self) -> Self {
        Self {
            glyphs: self.glyphs.compact(),
            ..self.clone()
        }
    }

    /// Legal but suspicious content: components whose base glyph does not exist.
    pub fn warnings(&self) -> Vec<Warning> {
        let mut warnings = Vec::new();
        for (glyph, data) in self.glyphs.iter() {
            for (layer, data) in &data.layers {
                for component in &data.components {
                    if self.glyphs.id_of(&component.base).is_none() {
                        warnings.push(Warning::DanglingComponent {
                            glyph,
                            layer: *layer,
                            base: component.base.clone(),
                        });
                    }
                }
            }
        }
        warnings
    }

    fn check_glyph(&self, glyph: &Glyph) -> Result<(), ModelError> {
        for (layer, data) in &glyph.layers {
            if self.layer_source(*layer).is_none() {
                return Err(ModelError::LayerNotFound(*layer));
            }
            check_glyph_layer(&glyph.name, *layer, data)?;
        }
        Ok(())
    }

    /// Layer IDs of every source except `except`.
    fn layer_ids_except(&self, except: Option<SourceId>) -> BTreeSet<LayerId> {
        self.sources
            .iter()
            .filter(|(id, _)| Some(**id) != except)
            .flat_map(|(_, source)| source.layers.iter().map(|layer| layer.id))
            .collect()
    }

    /// Whether the default layer, a designspace master or a glyph uses `layer`. Scans the
    /// glyph table, so it runs only when a source drops a layer.
    fn layer_in_use(&self, layer: LayerId) -> bool {
        layer == self.default_layer
            || self
                .designspace
                .iter()
                .flat_map(|d| &d.masters)
                .any(|m| m.layer == Some(layer))
            || self
                .glyphs
                .iter()
                .any(|(_, glyph)| glyph.layers.contains_key(&layer))
    }
}

/// Checks a source on its own and against the layer IDs other sources use: at least one
/// layer, unique layer IDs and names, no reserved keys.
fn check_source(
    id: SourceId,
    source: &Source,
    taken: &BTreeSet<LayerId>,
) -> Result<(), ModelError> {
    if source.layers.is_empty() {
        return Err(ModelError::NoLayers(id));
    }
    check_lib(&source.lib)?;
    if source.font_info.fields.contains_key(keys::GUIDELINES) {
        return Err(ModelError::ReservedKey(keys::GUIDELINES.to_owned()));
    }
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    for layer in &source.layers {
        check_lib(&layer.lib)?;
        if taken.contains(&layer.id) || !ids.insert(layer.id) {
            return Err(ModelError::DuplicateLayerId(layer.id));
        }
        if !names.insert(&layer.name) {
            return Err(ModelError::DuplicateLayerName {
                source_id: id,
                name: layer.name.clone(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Component, Contour, GlyphLayer, LibDict, LibValue, Master, Name, OBJECT_LIBS_KEY, Point,
        PointKind, SourceLayer, Transform,
    };

    fn name(s: &str) -> Name {
        Name::new(s).unwrap()
    }

    /// Source 0 with layers 0 (`public.default`) and 1 (`public.background`).
    fn font() -> Font {
        let mut source = Source::new(SourceLayer::new(LayerId(0), name("public.default")));
        source
            .layers
            .push(SourceLayer::new(LayerId(1), name("public.background")));
        Font::new(SourceId(0), source).unwrap()
    }

    fn glyph(glyph_name: &str, layers: &[(u32, GlyphLayer)]) -> Arc<Glyph> {
        let mut glyph = Glyph::new(name(glyph_name));
        for (layer, data) in layers {
            glyph.layers.insert(LayerId(*layer), Arc::new(data.clone()));
        }
        Arc::new(glyph)
    }

    fn square() -> GlyphLayer {
        let points = [(0.0, 0.0), (0.0, 100.0), (100.0, 100.0), (100.0, 0.0)]
            .into_iter()
            .map(|(x, y)| Point::new(x, y, PointKind::Line))
            .collect();
        GlyphLayer {
            width: 500.0,
            codepoints: ['A'].into_iter().collect(),
            contours: vec![Contour::new(points)],
            ..GlyphLayer::default()
        }
    }

    fn component_of(base: &str) -> GlyphLayer {
        GlyphLayer {
            components: vec![Component {
                base: name(base),
                transform: Transform::default(),
                identifier: None,
                lib: None,
            }],
            ..GlyphLayer::default()
        }
    }

    #[test]
    fn a_new_font_defaults_to_the_first_layer_of_its_source() {
        let font = font();
        assert_eq!(font.default_source(), SourceId(0));
        assert_eq!(font.default_layer(), LayerId(0));
        assert_eq!(font.layer_source(LayerId(1)), Some(SourceId(0)));
        assert_eq!(font.layer_source(LayerId(2)), None);
        assert!(font.glyphs().is_empty());
        assert!(font.designspace().is_none());
        assert_eq!(**font.project(), ProjectFiles::default());
        assert_eq!(font.sources().count(), 1);
        assert!(font.source(SourceId(0)).is_some());
        let empty = Source {
            layers: Vec::new(),
            ..Source::new(SourceLayer::new(LayerId(0), name("x")))
        };
        assert_eq!(
            Font::new(SourceId(3), empty),
            Err(ModelError::NoLayers(SourceId(3)))
        );
    }

    #[test]
    fn glyphs_are_checked_on_insert_and_replace() {
        let mut font = font();
        let id = font.insert_glyph(glyph("A", &[(0, square())])).unwrap();
        assert_eq!(font.glyph(id).unwrap().name.as_str(), "A");
        let before = font.clone();

        assert_eq!(
            font.insert_glyph(glyph("B", &[(7, square())])),
            Err(ModelError::LayerNotFound(LayerId(7)))
        );
        let mut open = square();
        open.contours[0].points[2].kind = PointKind::Move;
        assert!(matches!(
            font.insert_glyph(glyph("B", &[(1, open.clone())])),
            Err(ModelError::InvalidContour { contour: 0, .. })
        ));
        assert!(font.replace_glyph(id, glyph("A", &[(0, open)])).is_err());
        assert_eq!(
            font.insert_glyph(glyph("A", &[])),
            Err(ModelError::NameInUse(name("A")))
        );
        let mut reserved = square();
        reserved
            .lib
            .insert(OBJECT_LIBS_KEY.into(), LibValue::Dict(LibDict::new()));
        assert!(font.insert_glyph(glyph("B", &[(0, reserved)])).is_err());
        // Failed changes leave the font as it was.
        assert_eq!(font, before);

        let old = font
            .replace_glyph(id, glyph("A.ss01", &[(1, square())]))
            .unwrap();
        assert_eq!(old.name.as_str(), "A");
        assert_eq!(font.glyphs().id_of("A.ss01"), Some(id));
        assert_eq!(font.remove_glyph(id).unwrap().name.as_str(), "A.ss01");
        assert_eq!(font.remove_glyph(id), Err(ModelError::GlyphNotFound(id)));
    }

    #[test]
    fn a_clone_is_an_independent_snapshot() {
        let mut font = font();
        let id = font.insert_glyph(glyph("A", &[(0, square())])).unwrap();
        let snapshot = font.clone();
        let mut edited = (**font.glyph(id).unwrap()).clone();
        Arc::make_mut(edited.layers.get_mut(&LayerId(0)).unwrap()).width = 600.0;
        font.replace_glyph(id, Arc::new(edited)).unwrap();
        assert_eq!(snapshot.glyph(id).unwrap().layers[&LayerId(0)].width, 500.0);
        assert_eq!(font.glyph(id).unwrap().layers[&LayerId(0)].width, 600.0);
        assert_ne!(font, snapshot);
    }

    #[test]
    fn codepoints_are_a_view_on_the_default_layer() {
        let mut font = font();
        let a = font.insert_glyph(glyph("A", &[(0, square())])).unwrap();
        let background_only = font.insert_glyph(glyph("B", &[(1, square())])).unwrap();
        assert_eq!(font.codepoints(a), ['A']);
        assert!(font.codepoints(background_only).is_empty());
        assert!(font.codepoints(GlyphId(99)).is_empty());
        font.set_default_layer(LayerId(1)).unwrap();
        assert_eq!(font.codepoints(background_only), ['A']);
    }

    fn second_source() -> Source {
        Source::new(SourceLayer::new(LayerId(2), name("public.default")))
    }

    #[test]
    fn sources_need_unique_layer_ids_and_names() {
        let mut font = font();
        font.add_source(SourceId(1), second_source()).unwrap();
        assert_eq!(font.layer_source(LayerId(2)), Some(SourceId(1)));
        assert_eq!(
            font.add_source(SourceId(1), second_source()),
            Err(ModelError::SourceExists(SourceId(1)))
        );
        assert_eq!(
            font.add_source(SourceId(2), second_source()),
            Err(ModelError::DuplicateLayerId(LayerId(2)))
        );
        let mut same_name = Source::new(SourceLayer::new(LayerId(3), name("public.default")));
        same_name
            .layers
            .push(SourceLayer::new(LayerId(4), name("public.default")));
        assert_eq!(
            font.add_source(SourceId(2), same_name),
            Err(ModelError::DuplicateLayerName {
                source_id: SourceId(2),
                name: name("public.default")
            })
        );
        let mut twice = Source::new(SourceLayer::new(LayerId(3), name("a")));
        twice.layers.push(SourceLayer::new(LayerId(3), name("b")));
        assert_eq!(
            font.add_source(SourceId(2), twice),
            Err(ModelError::DuplicateLayerId(LayerId(3)))
        );
    }

    #[test]
    fn sources_refuse_reserved_keys() {
        let mut font = font();
        let mut lib_key = second_source();
        Arc::make_mut(&mut lib_key.lib).insert(OBJECT_LIBS_KEY.into(), LibValue::Boolean(true));
        assert!(font.add_source(SourceId(1), lib_key).is_err());

        let mut layer_key = second_source();
        layer_key.layers[0]
            .lib
            .insert(OBJECT_LIBS_KEY.into(), LibValue::Boolean(true));
        assert!(font.add_source(SourceId(1), layer_key).is_err());

        let mut info_key = second_source();
        Arc::make_mut(&mut info_key.font_info)
            .fields
            .insert(keys::GUIDELINES.into(), LibValue::Array(Vec::new()));
        assert_eq!(
            font.add_source(SourceId(1), info_key),
            Err(ModelError::ReservedKey(keys::GUIDELINES.into()))
        );
    }

    #[test]
    fn replacing_a_source_keeps_used_layers() {
        let mut font = font();
        font.insert_glyph(glyph("A", &[(1, square())])).unwrap();
        let original = Arc::clone(font.source(SourceId(0)).unwrap());

        // Dropping layer 1 (used by glyph A) or layer 0 (the default) is refused.
        let mut without_background = (*original).clone();
        without_background.layers.truncate(1);
        let without_background = Arc::new(without_background);
        assert_eq!(
            font.replace_source(SourceId(0), Arc::clone(&without_background)),
            Err(ModelError::LayerInUse(LayerId(1)))
        );
        let mut without_default = (*original).clone();
        without_default.layers.remove(0);
        assert_eq!(
            font.replace_source(SourceId(0), Arc::new(without_default)),
            Err(ModelError::LayerInUse(LayerId(0)))
        );
        assert_eq!(
            font.replace_source(SourceId(5), Arc::clone(&original)),
            Err(ModelError::SourceNotFound(SourceId(5)))
        );

        // Editing kerning keeps the layers: accepted.
        let mut kerned = (*original).clone();
        kerned.kerning.insert((name("A"), name("A")), -5.0);
        let old = font.replace_source(SourceId(0), Arc::new(kerned)).unwrap();
        assert_eq!(old, original);
        assert_eq!(font.source(SourceId(0)).unwrap().kerning.len(), 1);

        // Once nothing uses layer 1, it can go.
        font.remove_glyph(GlyphId(0)).unwrap();
        font.replace_source(SourceId(0), without_background)
            .unwrap();
        assert_eq!(font.layer_source(LayerId(1)), None);
    }

    fn master(source: u32, layer: Option<u32>) -> Master {
        Master {
            name: None,
            familyname: None,
            stylename: None,
            filename: "a.ufo".into(),
            source: SourceId(source),
            layer: layer.map(LayerId),
            location: Vec::new(),
        }
    }

    #[test]
    fn designspace_masters_point_at_existing_layers() {
        let mut font = font();
        font.add_source(SourceId(1), second_source()).unwrap();
        let ok = Designspace {
            masters: vec![master(0, None), master(0, Some(1)), master(1, Some(2))],
            ..Designspace::default()
        };
        font.set_designspace(Some(ok)).unwrap();
        assert_eq!(font.designspace().unwrap().masters.len(), 3);

        let unknown_source = Designspace {
            masters: vec![master(9, None)],
            ..Designspace::default()
        };
        assert_eq!(
            font.set_designspace(Some(unknown_source)),
            Err(ModelError::SourceNotFound(SourceId(9)))
        );
        let foreign_layer = Designspace {
            masters: vec![master(1, Some(1))],
            ..Designspace::default()
        };
        assert_eq!(
            font.set_designspace(Some(foreign_layer)),
            Err(ModelError::LayerNotFound(LayerId(1)))
        );
        // A master's layer cannot be dropped from its source.
        let mut without_background = (**font.source(SourceId(0)).unwrap()).clone();
        without_background.layers.truncate(1);
        assert_eq!(
            font.replace_source(SourceId(0), Arc::new(without_background)),
            Err(ModelError::LayerInUse(LayerId(1)))
        );
        font.set_designspace(None).unwrap();
        assert!(font.designspace().is_none());
    }

    #[test]
    fn default_layer_must_exist() {
        let mut font = font();
        font.add_source(SourceId(1), second_source()).unwrap();
        assert_eq!(
            font.set_default_layer(LayerId(9)),
            Err(ModelError::LayerNotFound(LayerId(9)))
        );
        font.set_default_layer(LayerId(2)).unwrap();
        assert_eq!(
            (font.default_source(), font.default_layer()),
            (SourceId(1), LayerId(2))
        );
    }

    #[test]
    fn dangling_components_are_warnings_not_errors() {
        let mut font = font();
        font.insert_glyph(glyph("A", &[(0, square())])).unwrap();
        let acute = font
            .insert_glyph(glyph(
                "Aacute",
                &[(0, component_of("A")), (1, component_of("acute"))],
            ))
            .unwrap();
        assert_eq!(
            font.warnings(),
            [Warning::DanglingComponent {
                glyph: acute,
                layer: LayerId(1),
                base: name("acute")
            }]
        );
    }

    #[test]
    fn compact_and_project_files() {
        let mut font = font();
        font.insert_glyph(glyph("A", &[(0, square())])).unwrap();
        let b = font.insert_glyph(glyph("B", &[(0, square())])).unwrap();
        font.remove_glyph(GlyphId(0)).unwrap();
        let compact = font.compact();
        assert_eq!(compact.glyphs().id_of("B"), Some(GlyphId(0)));
        assert_eq!(font.glyphs().id_of("B"), Some(b));
        assert_eq!(compact.sources().count(), 1);

        let mut files = BTreeMap::new();
        files.insert("assets/sketch.png".to_owned(), Arc::from(&b"png"[..]));
        font.set_project(ProjectFiles {
            id: Some("7f1c…".into()),
            files,
        });
        assert_eq!(font.project().files.len(), 1);
    }
}
