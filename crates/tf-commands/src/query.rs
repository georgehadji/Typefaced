//! The reads of the catalog (gate G4: 6 queries), their payloads and their responses.
//! A [`Query`] serializes as `{"id": "query.getGlyph", "args": {…}}`. The response of
//! `query.diff` (the sandbox `ChangeSet`) comes with Step 7.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::command::{
    GlyphId, KerningPair, KerningPairKey, KerningSide, LayerId, SourceId, Transform,
};
use crate::packed::PackedOutline;

/// An AI or MCP sandbox (ADR-0010).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type, JsonSchema)]
#[serde(transparent)]
pub struct SandboxId(pub u32);

/// The optional parts of a glyph layer that `query.getGlyph` returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum GlyphField {
    /// The contours, as a packed outline.
    Outline,
    /// The components.
    Components,
    /// The anchors.
    Anchors,
}

/// `query.fontSummary`: names, sources, axes and glyph count.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryFontSummary {}

/// `query.listGlyphs`: one page of glyphs in glyph order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryListGlyphs {
    /// Only glyphs whose name contains this text; `null` lists every glyph.
    pub filter: Option<String>,
    /// Number of matching glyphs to skip.
    pub offset: u32,
    /// Largest number of glyphs to return.
    pub limit: u32,
}

/// `query.getGlyph`: one glyph with its layers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryGetGlyph {
    /// The glyph.
    pub id: GlyphId,
    /// The optional parts to include; `null` includes all of them.
    pub fields: Option<Vec<GlyphField>>,
}

/// `query.kerning`: kerning pairs and kerning groups of one source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryKerning {
    /// The source; `null` means the default source.
    pub source: Option<SourceId>,
    /// Only these pairs; `null` returns every pair.
    pub pairs: Option<Vec<KerningPairKey>>,
}

/// `query.features`: the `features.fea` text of one source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryFeatures {
    /// The source; `null` means the default source.
    pub source: Option<SourceId>,
}

/// `query.diff`: the changes of a sandbox since it was forked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryDiff {
    /// The sandbox.
    pub sandbox: SandboxId,
}

catalog! {
    /// A read-only query (kind Q).
    Query, QueryId {
        /// `query.fontSummary`.
        FontSummary(QueryFontSummary) = "query.fontSummary",
        /// `query.listGlyphs`.
        ListGlyphs(QueryListGlyphs) = "query.listGlyphs",
        /// `query.getGlyph`.
        GetGlyph(QueryGetGlyph) = "query.getGlyph",
        /// `query.kerning`.
        Kerning(QueryKerning) = "query.kerning",
        /// `query.features`.
        Features(QueryFeatures) = "query.features",
        /// `query.diff`.
        Diff(QueryDiff) = "query.diff",
    }
}

/// A layer of a source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LayerSummary {
    /// The layer.
    pub id: LayerId,
    /// UFO layer name, e.g. `public.default`.
    pub name: String,
}

/// A source of the document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SourceSummary {
    /// The source.
    pub id: SourceId,
    /// Source name (the designspace source name, or the UFO's style name).
    pub name: String,
    /// Whether this is the default source.
    pub is_default: bool,
    /// The layers of the source's UFO.
    pub layers: Vec<LayerSummary>,
}

/// A designspace axis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AxisSummary {
    /// Axis name.
    pub name: String,
    /// Four-letter OpenType tag.
    pub tag: String,
    /// Smallest user value.
    pub minimum: f64,
    /// Default user value.
    pub default: f64,
    /// Largest user value.
    pub maximum: f64,
}

/// Response of `query.fontSummary`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FontSummary {
    /// `familyName` of the default source.
    pub family_name: Option<String>,
    /// `unitsPerEm` of the default source.
    pub units_per_em: Option<f64>,
    /// Number of glyphs.
    pub glyph_count: u32,
    /// The sources.
    pub sources: Vec<SourceSummary>,
    /// The axes; empty for a single UFO.
    pub axes: Vec<AxisSummary>,
}

/// A glyph in a list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GlyphSummary {
    /// The glyph.
    pub id: GlyphId,
    /// Glyph name.
    pub name: String,
    /// Unicode code points.
    pub codepoints: Vec<u32>,
}

/// Response of `query.listGlyphs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GlyphPage {
    /// Number of glyphs that match the filter.
    pub total: u32,
    /// The glyphs of the page.
    pub glyphs: Vec<GlyphSummary>,
}

/// A component in a glyph layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Component {
    /// Name of the base glyph.
    pub base: String,
    /// Placement of the base glyph.
    pub transform: Transform,
}

/// An anchor in a glyph layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Anchor {
    /// Anchor name (UFO allows anchors without one).
    pub name: Option<String>,
    /// X in font units.
    pub x: f64,
    /// Y in font units.
    pub y: f64,
}

/// One layer of a glyph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GlyphLayer {
    /// The layer.
    pub layer: LayerId,
    /// Advance width.
    pub width: f64,
    /// Advance height.
    pub height: f64,
    /// The contours, when requested.
    pub outline: Option<PackedOutline>,
    /// The components, when requested.
    pub components: Option<Vec<Component>>,
    /// The anchors, when requested.
    pub anchors: Option<Vec<Anchor>>,
}

/// Response of `query.getGlyph`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GlyphDetail {
    /// The glyph.
    pub id: GlyphId,
    /// Glyph name.
    pub name: String,
    /// Unicode code points.
    pub codepoints: Vec<u32>,
    /// Every layer of the glyph.
    pub layers: Vec<GlyphLayer>,
}

/// A kerning group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KerningGroup {
    /// The kerning side.
    pub side: KerningSide,
    /// Group name without the `public.kern1.`/`public.kern2.` prefix.
    pub name: String,
    /// Glyph names.
    pub members: Vec<String>,
}

/// Response of `query.kerning`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Kerning {
    /// The pairs.
    pub pairs: Vec<KerningPair>,
    /// Every kerning group of the source.
    pub groups: Vec<KerningGroup>,
}

/// Response of `query.features`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Features {
    /// The `features.fea` text; empty when the source has none.
    pub text: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_query_serializes_with_its_id_and_args() {
        let query = Query::GetGlyph(QueryGetGlyph {
            id: GlyphId(1),
            fields: Some(vec![GlyphField::Outline]),
        });

        assert_eq!(
            serde_json::to_value(&query).unwrap(),
            json!({ "id": "query.getGlyph", "args": { "id": 1, "fields": ["outline"] } })
        );
    }

    #[test]
    fn a_glyph_detail_carries_its_packed_outline_as_json() {
        let detail = GlyphDetail {
            id: GlyphId(2),
            name: "I".to_owned(),
            codepoints: vec![0x49],
            layers: vec![GlyphLayer {
                layer: LayerId(0),
                width: 200.0,
                height: 0.0,
                outline: Some(PackedOutline {
                    coords: vec![0.0, 0.0, 10.0, 0.0, 10.0, 10.0],
                    flags: vec![1, 1, 1],
                    contour_ends: vec![2],
                }),
                components: None,
                anchors: Some(vec![Anchor {
                    name: Some("top".to_owned()),
                    x: 5.0,
                    y: 10.0,
                }]),
            }],
        };

        let value = serde_json::to_value(&detail).unwrap();

        assert_eq!(value["layers"][0]["outline"]["contourEnds"], json!([2]));
        assert_eq!(
            serde_json::from_value::<GlyphDetail>(value).unwrap(),
            detail
        );
    }
}
