//! Checks a POST body before it is forwarded: one JSON object, no duplicate keys, only
//! the top-level fields the webview client sends, an `anthropic/<model>` ID and at most three
//! fallback models in OpenRouter's shape, the client's `max_tokens` and `thinking` limits,
//! no server-side tools or MCP servers, and no
//! content `source` the API would fetch from elsewhere (only inline `base64`, `text` and
//! `content` sources). The tests are in `policy.rs`, through `EgressPolicy::prepare`.

use std::collections::HashSet;

use serde::Deserialize;
use serde::de::{Error as _, MapAccess, SeqAccess, Visitor};

use crate::policy::PolicyError;

/// Content `source` types the API reads inline. Any other (`url`, `file`, …) makes the
/// API fetch something on the caller's behalf.
const INLINE_SOURCES: [&str; 3] = ["base64", "text", "content"];

/// Marks the walk error that means "a source the API would fetch".
const FETCHED_SOURCE: &str = "fetched content source";

/// The start of serde's `Error::unknown_field` text (serde 1.x), which
/// `deny_unknown_fields` raises after JSON escapes in the name are decoded. If the
/// wording ever changed, such bodies would be refused as `BadBody` instead.
const UNKNOWN_FIELD: &str = "unknown field";

/// Longest string kept by the walk; every `type` value is far shorter.
const MAX_TYPE_BYTES: usize = 64;

/// Most fallback models OpenRouter takes on `/api/v1/messages`.
const MAX_FALLBACKS: usize = 3;

/// Longest model ID accepted; real ones are about 40 bytes.
const MAX_MODEL_BYTES: usize = 100;

/// The only model vendor the client routes to (`packages/ai` `ROUTES`): Claude models.
/// Any other vendor, and OpenRouter's own routers, are refused (cost and data control).
const ALLOWED_VENDOR: &str = "anthropic";

/// The client's `max_tokens`; a larger value could only raise the cost.
const MAX_OUTPUT_TOKENS: u32 = 64_000;

/// Refuses bodies that ask for server-side tools, MCP servers or content the API would
/// fetch from elsewhere, bodies with a top-level field the client does not send, bodies
/// without an allowed model, and bodies that are not one JSON object without duplicate
/// keys. Returns the requested model, for the usage ledger.
pub(crate) fn check_body(body: Option<&str>) -> Result<String, PolicyError> {
    let body = body.ok_or(PolicyError::BadBody)?;
    // serde would also read a JSON array into a struct, by position.
    if !body.trim_start().starts_with('{') {
        return Err(PolicyError::BadBody);
    }
    if let Err(error) = serde_json::from_str::<Walk>(body) {
        let fetched = error.to_string().starts_with(FETCHED_SOURCE);
        return Err(if fetched {
            PolicyError::FetchedSource
        } else {
            PolicyError::BadBody
        });
    }
    let fields: BodyFields = serde_json::from_str(body).map_err(|error| {
        if error.to_string().starts_with(UNKNOWN_FIELD) {
            PolicyError::BodyField
        } else {
            PolicyError::BadBody
        }
    })?;
    let model = fields
        .model
        .0
        .filter(|m| model_allowed(m))
        .ok_or(PolicyError::Model)?;
    let fallbacks = fields.fallbacks.0.unwrap_or_default();
    if fallbacks.len() > MAX_FALLBACKS || !fallbacks.iter().all(|f| model_allowed(&f.model)) {
        return Err(PolicyError::Model);
    }
    // The client sends adaptive thinking only; a fixed budget could raise the cost.
    let thinking_allowed = fields
        .thinking
        .0
        .is_none_or(|t| t == serde_json::json!({ "type": "adaptive" }));
    if fields.max_tokens.0.is_some_and(|m| m > MAX_OUTPUT_TOKENS) || !thinking_allowed {
        return Err(PolicyError::Setting);
    }
    let server_tool = fields
        .tools
        .iter()
        .flatten()
        .any(|tool| match &tool.kind.0 {
            None => false,
            Some(kind) => kind.as_str() != Some("custom"),
        });
    if fields.mcp_servers.0.is_some() || server_tool {
        return Err(PolicyError::ServerTools);
    }
    Ok(model)
}

/// One OpenRouter model ID `anthropic/<model>`, the model part in lower case. No
/// `:variant` suffix: variants switch on OpenRouter features the policy refuses
/// elsewhere (`:online` is the web plugin; `:nitro`, `:floor` and `:free` pick
/// providers). Other vendors and OpenRouter's routers (`openrouter/...`) are refused.
fn model_allowed(id: &str) -> bool {
    id.len() <= MAX_MODEL_BYTES
        && id.split_once('/').is_some_and(|(vendor, model)| {
            vendor == ALLOWED_VENDOR
                && model
                    .bytes()
                    .next()
                    .is_some_and(|b| b.is_ascii_alphanumeric())
                && model
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
        })
}

/// A walk over any JSON value. It refuses duplicate keys in every object (the API might
/// read a different copy), and any object whose `source` is an object with a `type`
/// outside [`INLINE_SOURCES`]. It keeps only what the parent needs: the value if it is
/// a short string, whether it is an object, and an object's `type` if that is a string.
#[derive(Default)]
struct Walk {
    /// The value is a string, of any length.
    text: bool,
    string: Option<String>,
    object: bool,
    kind: Option<String>,
}

impl<'de> Deserialize<'de> for Walk {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(WalkVisitor)
    }
}

struct WalkVisitor;

impl<'de> Visitor<'de> for WalkVisitor {
    type Value = Walk;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Walk, A::Error> {
        let mut keys = HashSet::new();
        let mut kind = None;
        // Keys arrive with JSON escapes decoded.
        while let Some(key) = map.next_key::<String>()? {
            let value: Walk = map.next_value()?;
            // A string `source` (a citation's) is never fetched; an object one must be
            // inline; any other `source` (array, number, null) is refused too.
            let inline =
                value.object && INLINE_SOURCES.contains(&value.kind.as_deref().unwrap_or_default());
            if key == "source" && !value.text && !inline {
                return Err(A::Error::custom(FETCHED_SOURCE));
            }
            if key == "type" {
                kind = value.string;
            }
            if !keys.insert(key) {
                return Err(A::Error::custom("duplicate key"));
            }
        }
        Ok(Walk {
            object: true,
            kind,
            ..Walk::default()
        })
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Walk, A::Error> {
        while seq.next_element::<Walk>()?.is_some() {}
        Ok(Walk::default())
    }

    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Walk, E> {
        // Only short strings can be a `type`; long ones (base64 data) are not copied.
        Ok(Walk {
            text: true,
            string: (v.len() <= MAX_TYPE_BYTES).then(|| v.to_owned()),
            ..Walk::default()
        })
    }

    fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<Walk, E> {
        Ok(Walk::default())
    }

    fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<Walk, E> {
        Ok(Walk::default())
    }

    fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<Walk, E> {
        Ok(Walk::default())
    }

    fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<Walk, E> {
        Ok(Walk::default())
    }

    fn visit_unit<E: serde::de::Error>(self) -> Result<Walk, E> {
        Ok(Walk::default())
    }
}

/// The top-level fields of a messages body. Only the fields the webview client sends
/// (`packages/ai`) are allowed; any other is refused (`deny_unknown_fields`).
/// `mcp_servers` is listed only to be refused as a server feature; OpenRouter's own
/// extensions (`plugins`, `provider`, `models`, `route`, …) are not listed, so they are
/// refused like any other unknown field. Field matching sees
/// JSON escapes decoded, and a duplicate field is an error, so the API cannot read a
/// different copy than this check did.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[expect(dead_code, reason = "most fields exist only to be allowed; never read")]
struct BodyFields {
    #[serde(default)]
    mcp_servers: Present,
    #[serde(default)]
    tools: Option<Vec<ToolFields>>,
    #[serde(default)]
    model: Present<String>,
    #[serde(default)]
    max_tokens: Present<u32>,
    #[serde(default)]
    messages: Present,
    #[serde(default)]
    system: Present,
    #[serde(default)]
    thinking: Present<serde_json::Value>,
    #[serde(default)]
    output_config: Present,
    #[serde(default)]
    stream: Present,
    /// OpenRouter's fallback list (checked in `check_body`).
    #[serde(default)]
    fallbacks: Present<Vec<Fallback>>,
}

/// One OpenRouter fallback. Only `model`: per-attempt overrides could change the tools.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fallback {
    model: String,
}

#[derive(Deserialize)]
struct ToolFields {
    /// Absent for client tools; `"custom"` is the explicit client-tool type.
    #[serde(rename = "type", default)]
    kind: Present<serde_json::Value>,
}

/// Whether a field was present (with any value, `null` included), and its value.
#[derive(Default)]
struct Present<T = serde::de::IgnoredAny>(Option<T>);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Present<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        T::deserialize(deserializer).map(|value| Self(Some(value)))
    }
}
