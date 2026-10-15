//! The registry: one [`Descriptor`] per command and query (ADR-0004, implementation plan
//! §5.11, Appendix A), the `fontinfo.plist` keys `font.setInfo` accepts, and the strict
//! JSON Schema of each payload.
//!
//! Schemas follow the strict tool-schema shape of §5.11 already: payloads are inlined
//! (no `$defs`), every object has `additionalProperties: false` and lists every property
//! in `required`, and optional fields are nullable. The strict-mode contract test against
//! the Claude API limits comes in M3.

use schemars::transform::RecursiveTransform;
use schemars::{JsonSchema, Schema, generate::SchemaSettings};
use serde::Serialize;
use serde_json::Value;

use crate::command::CommandId;
use crate::query::QueryId;

/// Side-effect class (Appendix A).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Kind {
    /// Query: read-only, parallel-safe.
    Q,
    /// Mutation: undoable; runs in a sandbox for AI and MCP.
    M,
    /// External effect: writes outside the project; needs confirmation for AI and MCP.
    E,
    /// Destructive: bulk delete or overwrite; needs confirmation for AI and MCP.
    D,
}

/// A workspace profile that exposes a command (ADR-0014).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Workspace {
    /// The guided workspace for beginners.
    Studio,
    /// The workspace for professionals.
    Workbench,
}

/// Metadata of one command or query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Descriptor {
    /// Wire id, e.g. `glyph.create`.
    pub id: &'static str,
    /// Short title for menus and logs.
    pub title: &'static str,
    /// What it does, written for people and for the AI.
    pub description: &'static str,
    /// One complete example as JSON: `{"id": …, "args": {…}}`.
    pub example: &'static str,
    /// Side-effect class. `doc.saveAs` with `overwrite` is D at run time
    /// ([`crate::Command::kind`]).
    pub kind: Kind,
    /// The workspaces that expose it.
    pub workspaces: &'static [Workspace],
    /// Exposed as an AI and MCP tool.
    pub ai_exposed: bool,
    /// Can run in parallel with other calls (queries only).
    pub parallel_safe: bool,
}

const BOTH: &[Workspace] = &[Workspace::Studio, Workspace::Workbench];
const WORKBENCH: &[Workspace] = &[Workspace::Workbench];

const fn entry(
    id: &'static str,
    kind: Kind,
    workspaces: &'static [Workspace],
    ai_exposed: bool,
    title: &'static str,
    description: &'static str,
    example: &'static str,
) -> Descriptor {
    Descriptor {
        id,
        title,
        description,
        example,
        kind,
        workspaces,
        ai_exposed,
        parallel_safe: matches!(kind, Kind::Q),
    }
}

/// The descriptor of a command.
pub fn command(id: CommandId) -> Descriptor {
    use CommandId as C;
    use Kind::{D, E, M};
    let s = id.as_str();
    match id {
        C::DocNew => entry(
            s,
            E,
            BOTH,
            false,
            "New document",
            "Create a new document from a template.",
            r#"{"id":"doc.new","args":{"template":"blank"}}"#,
        ),
        C::DocOpen => entry(
            s,
            E,
            BOTH,
            false,
            "Open",
            "Open a .ufo folder, a .designspace file or a .typefaced package.",
            r#"{"id":"doc.open","args":{"path":"C:/Fonts/Inria.designspace"}}"#,
        ),
        C::DocSave => entry(
            s,
            E,
            BOTH,
            false,
            "Save",
            "Save the document where it was opened from.",
            r#"{"id":"doc.save","args":{}}"#,
        ),
        C::DocSaveAs => entry(
            s,
            E,
            BOTH,
            false,
            "Save as",
            "Save the document to a new path and format. Replacing an existing target needs overwrite: true and is destructive.",
            r#"{"id":"doc.saveAs","args":{"format":"typefaced","path":"C:/Fonts/Inria.typefaced","overwrite":false}}"#,
        ),
        C::DocClose => entry(
            s,
            E,
            BOTH,
            false,
            "Close",
            "Close the document. With unsaved changes, it fails unless discard is true.",
            r#"{"id":"doc.close","args":{"discard":false}}"#,
        ),
        C::DocUndo => entry(
            s,
            M,
            BOTH,
            false,
            "Undo",
            "Undo the last undo step.",
            r#"{"id":"doc.undo","args":{}}"#,
        ),
        C::DocRedo => entry(
            s,
            M,
            BOTH,
            false,
            "Redo",
            "Redo the last undone step.",
            r#"{"id":"doc.redo","args":{}}"#,
        ),
        C::GlyphCreate => entry(
            s,
            M,
            BOTH,
            true,
            "Create glyph",
            "Add an empty glyph with a unique name, its Unicode code points and an advance width, to every source.",
            r#"{"id":"glyph.create","args":{"name":"Aacute","codepoints":[193],"width":600}}"#,
        ),
        C::GlyphDelete => entry(
            s,
            D,
            BOTH,
            true,
            "Delete glyphs",
            "Delete glyphs. Fails for a glyph that other glyphs use as a component.",
            r#"{"id":"glyph.delete","args":{"ids":[12,13]}}"#,
        ),
        C::GlyphRename => entry(
            s,
            M,
            BOTH,
            true,
            "Rename glyph",
            "Rename a glyph and update components, kerning, groups and glyph order that refer to it.",
            r#"{"id":"glyph.rename","args":{"id":12,"name":"A.alt"}}"#,
        ),
        C::GlyphDuplicate => entry(
            s,
            M,
            BOTH,
            true,
            "Duplicate glyph",
            "Copy a glyph, with all its layers, under a new name. The copy has no code points.",
            r#"{"id":"glyph.duplicate","args":{"id":12,"name":"A.ss01"}}"#,
        ),
        C::GlyphSetCodepoints => entry(
            s,
            M,
            BOTH,
            true,
            "Set code points",
            "Replace the Unicode code points of a glyph. A code point another glyph has is refused.",
            r#"{"id":"glyph.setCodepoints","args":{"id":12,"codepoints":[65]}}"#,
        ),
        C::GlyphSetWidth => entry(
            s,
            M,
            BOTH,
            true,
            "Set width",
            "Set the advance width of a glyph in one layer.",
            r#"{"id":"glyph.setWidth","args":{"id":12,"layer":0,"width":640}}"#,
        ),
        C::GlyphSetOutline => entry(
            s,
            M,
            BOTH,
            true,
            "Set outline",
            "Replace the contours of a glyph layer with a packed outline: coords [x0, y0, …], one flag per point (0 off-curve, 1 line, 2 curve, 3 qcurve, +4 smooth) and the index of each contour's last point.",
            r#"{"id":"glyph.setOutline","args":{"id":12,"layer":0,"outline":{"coords":[0,0,500,0,250,700],"flags":[1,1,1],"contourEnds":[2]}}}"#,
        ),
        C::GlyphMovePoints => entry(
            s,
            M,
            BOTH,
            false,
            "Move points",
            "Move points of a glyph layer to new absolute positions, one coordinate pair per index.",
            r#"{"id":"glyph.movePoints","args":{"id":12,"layer":0,"indices":[0,2],"coords":[10,0,260,710]}}"#,
        ),
        C::GlyphInsertPoint => entry(
            s,
            M,
            BOTH,
            false,
            "Insert point",
            "Insert a point into a contour, before the point at the given position.",
            r#"{"id":"glyph.insertPoint","args":{"id":12,"layer":0,"contour":0,"index":1,"point":{"x":250,"y":0,"flags":1}}}"#,
        ),
        C::GlyphDeletePoints => entry(
            s,
            M,
            BOTH,
            false,
            "Delete points",
            "Delete points from a glyph layer.",
            r#"{"id":"glyph.deletePoints","args":{"id":12,"layer":0,"indices":[1]}}"#,
        ),
        C::GlyphToggleSmooth => entry(
            s,
            M,
            BOTH,
            false,
            "Toggle smooth",
            "Flip the smooth flag of on-curve points.",
            r#"{"id":"glyph.toggleSmooth","args":{"id":12,"layer":0,"indices":[2]}}"#,
        ),
        C::GlyphTransform => entry(
            s,
            M,
            BOTH,
            true,
            "Transform",
            "Apply an affine transform to a whole glyph layer, or to selected points, components and anchors.",
            r#"{"id":"glyph.transform","args":{"id":12,"layer":0,"matrix":{"xx":1,"xy":0,"yx":0.2,"yy":1,"dx":0,"dy":0},"selection":null}}"#,
        ),
        C::GlyphAddComponent => entry(
            s,
            M,
            BOTH,
            true,
            "Add component",
            "Add a component that draws another glyph, by name, with a transform.",
            r#"{"id":"glyph.addComponent","args":{"id":40,"layer":0,"base":"acute","transform":{"xx":1,"xy":0,"yx":0,"yy":1,"dx":180,"dy":0}}}"#,
        ),
        C::GlyphRemoveComponent => entry(
            s,
            M,
            BOTH,
            true,
            "Remove component",
            "Remove a component by its index.",
            r#"{"id":"glyph.removeComponent","args":{"id":40,"layer":0,"index":1}}"#,
        ),
        C::GlyphDecompose => entry(
            s,
            M,
            BOTH,
            true,
            "Decompose",
            "Replace components with the contours they draw, nested components included.",
            r#"{"id":"glyph.decompose","args":{"id":40,"layer":0,"indices":null}}"#,
        ),
        C::GlyphAddAnchor => entry(
            s,
            M,
            BOTH,
            true,
            "Add anchor",
            "Add a named anchor to a glyph layer.",
            r#"{"id":"glyph.addAnchor","args":{"id":12,"layer":0,"name":"top","x":300,"y":720}}"#,
        ),
        C::GlyphMoveAnchor => entry(
            s,
            M,
            BOTH,
            true,
            "Move anchor",
            "Move an anchor, by its index, to a new absolute position.",
            r#"{"id":"glyph.moveAnchor","args":{"id":12,"layer":0,"index":0,"x":310,"y":720}}"#,
        ),
        C::GlyphRemoveAnchor => entry(
            s,
            M,
            BOTH,
            true,
            "Remove anchor",
            "Remove an anchor by its index.",
            r#"{"id":"glyph.removeAnchor","args":{"id":12,"layer":0,"index":0}}"#,
        ),
        C::FontSetInfo => entry(
            s,
            M,
            BOTH,
            true,
            "Set font info",
            "Set or remove fontinfo.plist keys of one source, by their UFO key name; a null value removes the key.",
            r#"{"id":"font.setInfo","args":{"source":null,"fields":[{"key":"familyName","value":"Inria Sans"},{"key":"openTypeOS2WeightClass","value":400},{"key":"note","value":null}]}}"#,
        ),
        C::FontSetVerticalMetrics => entry(
            s,
            M,
            BOTH,
            true,
            "Set vertical metrics",
            "Set ascender, descender, x-height and cap height of one source; null leaves a value unchanged.",
            r#"{"id":"font.setVerticalMetrics","args":{"source":null,"ascender":750,"descender":-250,"xHeight":500,"capHeight":null}}"#,
        ),
        C::KerningSetPairs => entry(
            s,
            M,
            BOTH,
            true,
            "Set kerning pairs",
            "Add or change kerning pairs of one source. A side is a glyph name or a public.kern1./public.kern2. group.",
            r#"{"id":"kerning.setPairs","args":{"source":null,"pairs":[{"left":"T","right":"public.kern2.o","value":-60}]}}"#,
        ),
        C::KerningRemovePairs => entry(
            s,
            M,
            BOTH,
            true,
            "Remove kerning pairs",
            "Remove kerning pairs of one source.",
            r#"{"id":"kerning.removePairs","args":{"source":null,"pairs":[{"left":"T","right":"public.kern2.o"}]}}"#,
        ),
        C::KerningSetGroup => entry(
            s,
            M,
            BOTH,
            true,
            "Set kerning group",
            "Set the members of a kerning group of one source; empty members remove the group.",
            r#"{"id":"kerning.setGroup","args":{"source":null,"side":"second","name":"o","members":["o","oacute","ograve"]}}"#,
        ),
        C::FeaturesSetCode => entry(
            s,
            M,
            WORKBENCH,
            true,
            "Set feature code",
            "Replace the OpenType feature code (features.fea) of one source.",
            r#"{"id":"features.setCode","args":{"source":null,"text":"languagesystem DFLT dflt;\n"}}"#,
        ),
    }
}

/// The descriptor of a query.
pub fn query(id: QueryId) -> Descriptor {
    use Kind::Q;
    use QueryId as Qu;
    let s = id.as_str();
    match id {
        Qu::FontSummary => entry(
            s,
            Q,
            BOTH,
            true,
            "Font summary",
            "Family name, units per em, glyph count, sources with their layers, and axes.",
            r#"{"id":"query.fontSummary","args":{}}"#,
        ),
        Qu::ListGlyphs => entry(
            s,
            Q,
            BOTH,
            true,
            "List glyphs",
            "One page of glyphs (id, name, code points) in glyph order, optionally only names that contain a text.",
            r#"{"id":"query.listGlyphs","args":{"filter":"acute","offset":0,"limit":100}}"#,
        ),
        Qu::GetGlyph => entry(
            s,
            Q,
            BOTH,
            true,
            "Get glyph",
            "One glyph with every layer: width and height, plus the outline, components and anchors when requested.",
            r#"{"id":"query.getGlyph","args":{"id":12,"fields":["outline","anchors"]}}"#,
        ),
        Qu::Kerning => entry(
            s,
            Q,
            BOTH,
            true,
            "Kerning",
            "Kerning pairs (all, or the ones asked for) and kerning groups of one source.",
            r#"{"id":"query.kerning","args":{"source":null,"pairs":[{"left":"T","right":"o"}]}}"#,
        ),
        Qu::Features => entry(
            s,
            Q,
            BOTH,
            true,
            "Feature code",
            "The OpenType feature code (features.fea) of one source.",
            r#"{"id":"query.features","args":{"source":null}}"#,
        ),
        Qu::Diff => entry(
            s,
            Q,
            BOTH,
            false,
            "Sandbox diff",
            "The changes a sandbox made since it was forked.",
            r#"{"id":"query.diff","args":{"sandbox":1}}"#,
        ),
    }
}

/// The value type of a `fontinfo.plist` key, as norad 0.18.4's `FontInfo` holds it.
/// Step 5 checks values against it, and the ranges and enumerations of the UFO spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfoValueType {
    /// A string.
    String,
    /// A 32-bit signed integer.
    Integer,
    /// A non-negative integer.
    NonNegativeInteger,
    /// An integer or a real number.
    Number,
    /// A non-negative integer or real number.
    NonNegativeNumber,
    /// A boolean.
    Boolean,
    /// A list of integers (bit lists, Panose, family class).
    IntegerList,
    /// A list of integers or real numbers.
    NumberList,
}

/// The `fontinfo.plist` keys `font.setInfo` accepts: every key of norad 0.18.4's
/// `FontInfo` whose value is a scalar or a list of numbers. Structured keys
/// (`guidelines`, `openTypeGaspRangeRecords`, `openTypeNameRecords` and the nine
/// `woffMetadata*` records) are refused with `E_INVALID_FIELD`; they stay as loaded.
pub const FONT_INFO_KEYS: &[(&str, InfoValueType)] = {
    use InfoValueType::*;
    &[
        ("ascender", Number),
        ("capHeight", Number),
        ("copyright", String),
        ("descender", Number),
        ("familyName", String),
        ("italicAngle", Number),
        ("macintoshFONDFamilyID", Integer),
        ("macintoshFONDName", String),
        ("note", String),
        ("openTypeHeadCreated", String),
        ("openTypeHeadFlags", IntegerList),
        ("openTypeHeadLowestRecPPEM", NonNegativeInteger),
        ("openTypeHheaAscender", Integer),
        ("openTypeHheaCaretOffset", Integer),
        ("openTypeHheaCaretSlopeRise", Integer),
        ("openTypeHheaCaretSlopeRun", Integer),
        ("openTypeHheaDescender", Integer),
        ("openTypeHheaLineGap", Integer),
        ("openTypeNameCompatibleFullName", String),
        ("openTypeNameDescription", String),
        ("openTypeNameDesignerURL", String),
        ("openTypeNameDesigner", String),
        ("openTypeNameLicense", String),
        ("openTypeNameLicenseURL", String),
        ("openTypeNameManufacturer", String),
        ("openTypeNameManufacturerURL", String),
        ("openTypeNamePreferredFamilyName", String),
        ("openTypeNamePreferredSubfamilyName", String),
        ("openTypeNameSampleText", String),
        ("openTypeNameUniqueID", String),
        ("openTypeNameVersion", String),
        ("openTypeNameWWSFamilyName", String),
        ("openTypeNameWWSSubfamilyName", String),
        ("openTypeOS2CodePageRanges", IntegerList),
        ("openTypeOS2FamilyClass", IntegerList),
        ("openTypeOS2Panose", IntegerList),
        ("openTypeOS2Selection", IntegerList),
        ("openTypeOS2StrikeoutPosition", Integer),
        ("openTypeOS2StrikeoutSize", Integer),
        ("openTypeOS2SubscriptXOffset", Integer),
        ("openTypeOS2SubscriptXSize", Integer),
        ("openTypeOS2SubscriptYOffset", Integer),
        ("openTypeOS2SubscriptYSize", Integer),
        ("openTypeOS2SuperscriptXOffset", Integer),
        ("openTypeOS2SuperscriptXSize", Integer),
        ("openTypeOS2SuperscriptYOffset", Integer),
        ("openTypeOS2SuperscriptYSize", Integer),
        ("openTypeOS2Type", IntegerList),
        ("openTypeOS2TypoAscender", Integer),
        ("openTypeOS2TypoDescender", Integer),
        ("openTypeOS2TypoLineGap", Integer),
        ("openTypeOS2UnicodeRanges", IntegerList),
        ("openTypeOS2VendorID", String),
        ("openTypeOS2WeightClass", NonNegativeInteger),
        ("openTypeOS2WidthClass", Integer),
        ("openTypeOS2WinAscent", NonNegativeInteger),
        ("openTypeOS2WinDescent", NonNegativeInteger),
        ("openTypeVheaCaretOffset", Integer),
        ("openTypeVheaCaretSlopeRise", Integer),
        ("openTypeVheaCaretSlopeRun", Integer),
        ("openTypeVheaVertTypoAscender", Integer),
        ("openTypeVheaVertTypoDescender", Integer),
        ("openTypeVheaVertTypoLineGap", Integer),
        ("postscriptBlueFuzz", Number),
        ("postscriptBlueScale", Number),
        ("postscriptBlueShift", Number),
        ("postscriptBlueValues", NumberList),
        ("postscriptDefaultCharacter", String),
        ("postscriptDefaultWidthX", Number),
        ("postscriptFamilyBlues", NumberList),
        ("postscriptFamilyOtherBlues", NumberList),
        ("postscriptFontName", String),
        ("postscriptForceBold", Boolean),
        ("postscriptFullName", String),
        ("postscriptIsFixedPitch", Boolean),
        ("postscriptNominalWidthX", Number),
        ("postscriptOtherBlues", NumberList),
        ("postscriptSlantAngle", Number),
        ("postscriptStemSnapH", NumberList),
        ("postscriptStemSnapV", NumberList),
        ("postscriptUnderlinePosition", Number),
        ("postscriptUnderlineThickness", Number),
        ("postscriptUniqueID", Integer),
        ("postscriptWeightName", String),
        ("postscriptWindowsCharacterSet", Integer),
        ("styleMapFamilyName", String),
        ("styleMapStyleName", String),
        ("styleName", String),
        ("trademark", String),
        ("unitsPerEm", NonNegativeNumber),
        ("versionMajor", Integer),
        ("versionMinor", NonNegativeInteger),
        ("woffMajorVersion", NonNegativeInteger),
        ("woffMinorVersion", NonNegativeInteger),
        ("xHeight", Number),
        ("year", Integer),
    ]
};

/// The strict JSON Schema of a payload type (see the module documentation).
pub fn strict_schema<T: JsonSchema>() -> Schema {
    SchemaSettings::draft2020_12()
        .with(|settings| settings.inline_subschemas = true)
        .with_transform(RecursiveTransform(close_object))
        .into_generator()
        .into_root_schema_for::<T>()
}

/// Makes an object schema strict: no extra properties, every property required (sorted,
/// so the order never depends on serde_json's map features).
fn close_object(schema: &mut Schema) {
    let Some(object) = schema.as_object_mut() else {
        return;
    };
    if !object.get("type").is_some_and(allows_object) {
        return;
    }
    let mut keys: Vec<String> = object
        .get("properties")
        .and_then(Value::as_object)
        .map(|properties| properties.keys().cloned().collect())
        .unwrap_or_default();
    keys.sort();
    object.insert(
        "required".to_owned(),
        Value::Array(keys.into_iter().map(Value::String).collect()),
    );
    object.insert("additionalProperties".to_owned(), Value::Bool(false));
}

/// Whether a `type` keyword (a name or a list of names) allows objects.
fn allows_object(kind: &Value) -> bool {
    match kind {
        Value::String(name) => name == "object",
        Value::Array(names) => names.iter().any(allows_object),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Command, Query};
    use std::collections::{BTreeSet, HashSet};

    fn all() -> Vec<(Descriptor, Schema)> {
        CommandId::ALL
            .iter()
            .map(|&id| (command(id), id.schema()))
            .chain(QueryId::ALL.iter().map(|&id| (query(id), id.schema())))
            .collect()
    }

    #[test]
    fn the_catalog_has_31_commands_and_6_queries_with_unique_ids() {
        let ids: HashSet<&str> = all().iter().map(|(d, _)| d.id).collect();

        assert_eq!(CommandId::ALL.len(), 31);
        assert_eq!(QueryId::ALL.len(), 6);
        assert_eq!(ids.len(), 37);
    }

    #[test]
    fn every_descriptor_carries_its_own_id_and_text() {
        for &id in CommandId::ALL {
            assert_eq!(command(id).id, id.as_str());
        }
        for &id in QueryId::ALL {
            assert_eq!(query(id).id, id.as_str());
        }
        for (d, _) in all() {
            assert!(!d.title.is_empty() && !d.description.is_empty(), "{}", d.id);
            assert!(!d.workspaces.is_empty(), "{}", d.id);
        }
    }

    #[test]
    fn every_command_example_parses_into_its_own_variant_and_round_trips() {
        for &id in CommandId::ALL {
            let example: Value = serde_json::from_str(command(id).example).unwrap();

            let parsed: Command = serde_json::from_value(example).unwrap();
            let json = serde_json::to_value(&parsed).unwrap();

            assert_eq!(parsed.id(), id);
            assert_eq!(serde_json::from_value::<Command>(json).unwrap(), parsed);
        }
    }

    #[test]
    fn every_query_example_parses_into_its_own_variant_and_round_trips() {
        for &id in QueryId::ALL {
            let example: Value = serde_json::from_str(query(id).example).unwrap();

            let parsed: Query = serde_json::from_value(example).unwrap();
            let json = serde_json::to_value(&parsed).unwrap();

            assert_eq!(parsed.id(), id);
            assert_eq!(serde_json::from_value::<Query>(json).unwrap(), parsed);
        }
    }

    #[test]
    fn every_example_lists_exactly_the_properties_its_schema_requires() {
        for (d, schema) in all() {
            let example: Value = serde_json::from_str(d.example).unwrap();
            let args: BTreeSet<&String> = example["args"].as_object().unwrap().keys().collect();
            let required: BTreeSet<&String> = schema
                .get("required")
                .and_then(Value::as_array)
                .unwrap()
                .iter()
                .filter_map(|v| match v {
                    Value::String(s) => Some(s),
                    _ => None,
                })
                .collect();

            assert_eq!(args, required, "{}", d.id);
        }
    }

    #[test]
    fn kinds_and_flags_follow_appendix_a() {
        let kind = |id| command(id).kind;
        for &id in CommandId::ALL {
            let expected = match id {
                CommandId::DocNew
                | CommandId::DocOpen
                | CommandId::DocSave
                | CommandId::DocSaveAs
                | CommandId::DocClose => Kind::E,
                CommandId::GlyphDelete => Kind::D,
                _ => Kind::M,
            };
            assert_eq!(kind(id), expected, "{}", id.as_str());
            assert!(!command(id).parallel_safe);
        }
        for &id in QueryId::ALL {
            assert_eq!(query(id).kind, Kind::Q);
            assert!(query(id).parallel_safe);
        }
        let ui_only = [
            CommandId::GlyphMovePoints,
            CommandId::GlyphInsertPoint,
            CommandId::GlyphDeletePoints,
            CommandId::GlyphToggleSmooth,
        ];
        for id in ui_only {
            assert!(!command(id).ai_exposed, "{}", id.as_str());
        }
        assert!(command(CommandId::GlyphTransform).ai_exposed);
        assert!(!query(QueryId::Diff).ai_exposed);
    }

    #[test]
    fn every_schema_is_strict() {
        fn check(value: &Value, at: &str) {
            if let Some(object) = value.as_object() {
                if object.get("type").is_some_and(allows_object) {
                    let mut keys: Vec<&String> = object
                        .get("properties")
                        .and_then(Value::as_object)
                        .map(|p| p.keys().collect())
                        .unwrap_or_default();
                    keys.sort();
                    let required: Vec<&String> = object["required"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| match v {
                            Value::String(s) => s,
                            other => panic!("{other} in required at {at}"),
                        })
                        .collect();
                    assert_eq!(required, keys, "{at}");
                    assert_eq!(object["additionalProperties"], Value::Bool(false), "{at}");
                }
                assert!(!object.contains_key("$defs"), "{at}");
                assert!(!object.contains_key("$ref"), "{at}");
                object.values().for_each(|v| check(v, at));
            } else if let Some(array) = value.as_array() {
                array.iter().for_each(|v| check(v, at));
            }
        }
        for (d, schema) in all() {
            assert_eq!(schema.get("type"), Some(&Value::from("object")), "{}", d.id);
            check(schema.as_value(), d.id);
        }
    }

    #[test]
    fn the_info_keys_cover_every_scalar_key_of_norad_fontinfo_once() {
        let keys: HashSet<&str> = FONT_INFO_KEYS.iter().map(|(k, _)| *k).collect();

        // norad 0.18.4's FontInfo has 108 keys; 12 are structured and refused.
        assert_eq!(FONT_INFO_KEYS.len(), 96);
        assert_eq!(keys.len(), FONT_INFO_KEYS.len());
        assert!(!keys.contains("guidelines") && !keys.contains("openTypeNameRecords"));
    }

    #[test]
    fn descriptors_serialize_for_code_generation() {
        let value = serde_json::to_value(command(CommandId::GlyphDelete)).unwrap();

        assert_eq!(value["kind"], "D");
        assert_eq!(
            value["workspaces"],
            serde_json::json!(["studio", "workbench"])
        );
        assert_eq!(value["aiExposed"], true);
        assert_eq!(value["parallelSafe"], false);
    }

    #[test]
    fn every_payload_schema_matches_its_snapshot() {
        let mut settings = insta::Settings::clone_current();
        settings.set_sort_maps(true);
        settings.set_prepend_module_to_snapshot(false);
        settings.set_omit_expression(true);
        settings.bind(|| {
            for (d, schema) in all() {
                insta::assert_json_snapshot!(d.id, schema);
            }
        });
    }
}
