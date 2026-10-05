//! Checks a POST body before it is forwarded: one JSON object, no duplicate keys, no
//! server-side tools or MCP servers, and no content `source` the API would fetch from
//! elsewhere (only inline `base64`, `text` and `content` sources). The tests are in
//! `policy.rs`, through `EgressPolicy::prepare`.

use std::collections::HashSet;

use serde::Deserialize;
use serde::de::{Error as _, MapAccess, SeqAccess, Visitor};

use crate::policy::PolicyError;

/// Content `source` types the API reads inline. Any other (`url`, `file`, …) makes the
/// API fetch something on the caller's behalf.
const INLINE_SOURCES: [&str; 3] = ["base64", "text", "content"];

/// Marks the walk error that means "a source the API would fetch".
const FETCHED_SOURCE: &str = "fetched content source";

/// Longest string kept by the walk; every `type` value is far shorter.
const MAX_TYPE_BYTES: usize = 64;

/// Refuses bodies that ask for server-side tools, MCP servers or content the API would
/// fetch from elsewhere, and bodies that are not one JSON object without duplicate keys.
pub(crate) fn check_body(body: Option<&str>) -> Result<(), PolicyError> {
    let body = body.ok_or(PolicyError::BadBody)?;
    // serde would also read a JSON array into a struct, by position.
    if !body.trim_start().starts_with('{') {
        return Err(PolicyError::BadBody);
    }
    if let Err(error) = serde_json::from_str::<Walk>(body) {
        let fetched = error.to_string().starts_with(FETCHED_SOURCE);
        return Err(if fetched {
            PolicyError::ServerTools
        } else {
            PolicyError::BadBody
        });
    }
    let fields: BodyFields = serde_json::from_str(body).map_err(|_| PolicyError::BadBody)?;
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
    Ok(())
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

/// The top-level fields of a messages body that can turn on server-side work. Field
/// matching sees JSON escapes decoded, and a duplicate field is an error, so the API
/// cannot read a different copy than this check did.
#[derive(Deserialize)]
struct BodyFields {
    #[serde(default)]
    mcp_servers: Present,
    #[serde(default)]
    tools: Option<Vec<ToolFields>>,
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
