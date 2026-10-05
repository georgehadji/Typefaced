//! `EgressPolicy`: decides whether a request from the webview may leave the machine, and
//! rebuilds it with only the allowed parts: one origin, four endpoints, client tools only,
//! a size limit and a header allowlist.

use reqwest::Method;
use reqwest::Url;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::Deserialize;
use specta::Type;

/// The only destination in M0: the Claude API.
pub const ANTHROPIC_ORIGIN: &str = "https://api.anthropic.com";

/// Upper bound for a request body: 32 MiB.
pub const MAX_BODY_BYTES: usize = 32 * 1024 * 1024;

/// Request headers that pass through. Every other header is dropped, including
/// `x-api-key`, `authorization` and `cookie`; the host adds `x-api-key` itself.
const PASSED_HEADERS: [&str; 4] = [
    "anthropic-version",
    "anthropic-beta",
    "content-type",
    "accept",
];

/// A request as the webview's `fetch` describes it, before any check.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProxyRequest {
    /// Chosen by the caller (e.g. a UUID); `ai_abort` cancels the request by this ID.
    pub request_id: String,
    /// `GET` or `POST`.
    pub method: String,
    /// Absolute URL, e.g. `https://api.anthropic.com/v1/messages`.
    pub url: String,
    /// Header name and value pairs.
    pub headers: Vec<(String, String)>,
    /// UTF-8 body, if any (the Claude API takes JSON).
    pub body: Option<String>,
}

/// A request that passed the policy: ready to send once the key is added.
#[derive(Debug)]
pub(crate) struct PreparedRequest {
    pub method: Method,
    pub url: Url,
    pub headers: HeaderMap,
    pub body: Option<String>,
}

/// Why the policy refused a request. The messages never echo header values or bodies.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PolicyError {
    #[error("only GET and POST requests are forwarded")]
    Method,
    #[error("the URL is not a valid absolute URL")]
    BadUrl,
    #[error(
        "the destination is not allowed: only POST /v1/messages and /v1/messages/count_tokens \
         and GET /v1/models[/{{id}}] on {ANTHROPIC_ORIGIN} are"
    )]
    Destination,
    #[error("a forwarded header has an invalid name or value")]
    BadHeader,
    #[error("the request body is larger than {} MiB", MAX_BODY_BYTES >> 20)]
    BodyTooLarge,
    #[error("a GET request cannot have a body")]
    BodyOnGet,
    #[error("the request body must be one JSON object without duplicate `tools` or `mcp_servers`")]
    BadBody,
    #[error("server-side tools and MCP servers are not allowed; only client tools are")]
    ServerTools,
}

/// The egress rules: one allowed origin and the endpoints in `endpoint_allowed`.
#[derive(Debug, Clone)]
pub struct EgressPolicy {
    origin: Option<Url>,
}

impl EgressPolicy {
    /// The production policy: only the Claude API at `https://api.anthropic.com`.
    pub fn anthropic() -> Self {
        Self::for_origin(ANTHROPIC_ORIGIN)
    }

    /// A policy for another origin. Only tests use it, to point at a local mock server.
    #[cfg(test)]
    pub(crate) fn for_test_origin(origin: &str) -> Self {
        Self::for_origin(origin)
    }

    /// An unparsable origin matches nothing, so the policy fails closed.
    fn for_origin(origin: &str) -> Self {
        Self {
            origin: Url::parse(origin).ok(),
        }
    }

    /// Checks `request` and rebuilds it with only the allowed parts. Takes the request
    /// by value so a large body is moved, never copied.
    pub(crate) fn prepare(&self, request: ProxyRequest) -> Result<PreparedRequest, PolicyError> {
        let method = allowed_method(&request.method)?;
        let url = self.allowed_url(&request.url)?;
        if request
            .body
            .as_ref()
            .is_some_and(|b| b.len() > MAX_BODY_BYTES)
        {
            return Err(PolicyError::BodyTooLarge);
        }
        if method == Method::GET && request.body.is_some() {
            return Err(PolicyError::BodyOnGet);
        }
        if !endpoint_allowed(&method, url.path()) {
            return Err(PolicyError::Destination);
        }
        // Only the two POST endpoints take a body; parsed only after the checks above.
        if method == Method::POST {
            check_body(request.body.as_deref())?;
        }
        Ok(PreparedRequest {
            method,
            url,
            headers: passed_headers(&request.headers)?,
            body: request.body,
        })
    }

    fn allowed_url(&self, raw: &str) -> Result<Url, PolicyError> {
        // Parsing normalises the host (lower case) and the path (`.` and `..` segments,
        // also percent-encoded), so the checks below see what will be sent.
        let mut url = Url::parse(raw).map_err(|_| PolicyError::BadUrl)?;
        let Some(origin) = &self.origin else {
            return Err(PolicyError::Destination);
        };
        let same_origin = url.scheme() == origin.scheme()
            && url.host_str() == origin.host_str()
            && url.port_or_known_default() == origin.port_or_known_default();
        let no_credentials = url.username().is_empty() && url.password().is_none();
        if !same_origin || !no_credentials {
            return Err(PolicyError::Destination);
        }
        // A fragment is never sent over HTTP anyway.
        url.set_fragment(None);
        Ok(url)
    }
}

/// The endpoints the SDK needs, matched exactly on the normalised path (the query is not
/// part of it): POST `/v1/messages` and `/v1/messages/count_tokens`, GET `/v1/models`
/// and `/v1/models/{id}`.
fn endpoint_allowed(method: &Method, path: &str) -> bool {
    if *method == Method::POST {
        return matches!(path, "/v1/messages" | "/v1/messages/count_tokens");
    }
    if path == "/v1/models" {
        return true;
    }
    path.strip_prefix("/v1/models/").is_some_and(|id| {
        !id.is_empty()
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    })
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

/// Refuses bodies that ask for server-side tools or MCP servers, and bodies that are not
/// one JSON object.
fn check_body(body: Option<&str>) -> Result<(), PolicyError> {
    let body = body.ok_or(PolicyError::BadBody)?;
    // serde would also read a JSON array into the struct, by position.
    if !body.trim_start().starts_with('{') {
        return Err(PolicyError::BadBody);
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

fn allowed_method(raw: &str) -> Result<Method, PolicyError> {
    if raw.eq_ignore_ascii_case("POST") {
        Ok(Method::POST)
    } else if raw.eq_ignore_ascii_case("GET") {
        Ok(Method::GET)
    } else {
        Err(PolicyError::Method)
    }
}

fn passed_headers(raw: &[(String, String)]) -> Result<HeaderMap, PolicyError> {
    let mut headers = HeaderMap::new();
    for (name, value) in raw {
        let Some(allowed) = PASSED_HEADERS
            .iter()
            .find(|allowed| name.eq_ignore_ascii_case(allowed))
        else {
            continue;
        };
        let value = HeaderValue::from_str(value).map_err(|_| PolicyError::BadHeader)?;
        headers.append(HeaderName::from_static(allowed), value);
    }
    Ok(headers)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(method: &str, url: &str) -> ProxyRequest {
        ProxyRequest {
            request_id: "r1".into(),
            method: method.into(),
            url: url.into(),
            headers: Vec::new(),
            body: None,
        }
    }

    fn post(url: &str) -> ProxyRequest {
        ProxyRequest {
            body: Some("{}".into()),
            ..request("POST", url)
        }
    }

    #[test]
    fn allows_post_and_get_to_the_claude_api() {
        let policy = EgressPolicy::anthropic();
        let prepared = policy
            .prepare(post("https://api.anthropic.com/v1/messages?beta=true"))
            .unwrap();
        assert_eq!(prepared.method, Method::POST);
        assert_eq!(
            prepared.url.as_str(),
            "https://api.anthropic.com/v1/messages?beta=true"
        );
        assert_eq!(prepared.body.as_deref(), Some("{}"));

        let get = policy
            .prepare(request("get", "https://API.anthropic.com:443/v1/models"))
            .unwrap();
        assert_eq!(get.method, Method::GET);
        assert_eq!(get.url.as_str(), "https://api.anthropic.com/v1/models");
    }

    #[test]
    fn rejects_every_other_destination() {
        let policy = EgressPolicy::anthropic();
        for url in [
            "http://api.anthropic.com/v1/messages",
            "https://api.anthropic.com:8443/v1/messages",
            "https://api.anthropic.com./v1/messages",
            "https://evil.example/v1/messages",
            "https://api.anthropic.com.evil.example/v1/messages",
            "https://evil.example@api.anthropic.com/v1/messages",
            "https://user:pass@api.anthropic.com/v1/messages",
            "https://api.anthropic.com/v2/messages",
            "https://api.anthropic.com/v1",
            "https://api.anthropic.com/",
            "https://api.anthropic.com/v1/../admin",
            "https://api.anthropic.com/v1/%2e%2e/admin",
            "https://api.anthropic.com/v1/..\\admin",
        ] {
            assert_eq!(
                policy.prepare(post(url)).unwrap_err(),
                PolicyError::Destination,
                "{url}"
            );
        }
    }

    #[test]
    fn allows_only_the_endpoints_the_sdk_needs() {
        let policy = EgressPolicy::anthropic();
        let base = "https://api.anthropic.com";
        for path in [
            "/v1/messages",
            "/v1/messages/count_tokens",
            "/v1/messages?beta=true",
        ] {
            assert!(
                policy.prepare(post(&format!("{base}{path}"))).is_ok(),
                "{path}"
            );
        }
        for path in [
            "/v1/models",
            "/v1/models/claude-opus-5",
            "/v1/models/claude-3.5_x-1",
        ] {
            let found = policy.prepare(request("GET", &format!("{base}{path}")));
            assert!(found.is_ok(), "{path}");
        }
    }

    #[test]
    fn refuses_every_other_endpoint_and_method_pairing() {
        let policy = EgressPolicy::anthropic();
        let base = "https://api.anthropic.com";
        for path in [
            "/v1/files",
            "/v1/files/file_1/content",
            "/v1/messages/batches",
            "/v1/messages/batches/b1/results",
            "/v1/messages/../files",
            "/v1/messages/%2e%2e/files",
            "/v1/messages/",
            "/v1/messages/count_tokens/",
            "/v1/Messages",
            "/v1/complete",
            "/v1/models",
            "/v1/skills",
            "/v1/organizations/me",
        ] {
            assert_eq!(
                policy.prepare(post(&format!("{base}{path}"))).unwrap_err(),
                PolicyError::Destination,
                "POST {path}"
            );
        }
        for path in [
            "/v1/messages",
            "/v1/messages/count_tokens",
            "/v1/models/",
            "/v1/models/a/b",
            "/v1/models/%2e",
            "/v1/models/a%2Fb",
            "/v1/files",
            "/v1/messages/batches",
        ] {
            let found = policy.prepare(request("GET", &format!("{base}{path}")));
            assert_eq!(found.unwrap_err(), PolicyError::Destination, "GET {path}");
        }
    }

    fn post_body(body: &str) -> ProxyRequest {
        ProxyRequest {
            body: Some(body.into()),
            ..request("POST", "https://api.anthropic.com/v1/messages")
        }
    }

    #[test]
    fn allows_client_tools_and_custom_tools() {
        let policy = EgressPolicy::anthropic();
        for body in [
            r#"{"model":"m","messages":[]}"#,
            r#"{"model":"m","tools":[]}"#,
            r#"  {"tools":[{"name":"get_font_summary","input_schema":{"type":"object"}}]}"#,
            r#"{"tools":[{"type":"custom","name":"t","input_schema":{}}]}"#,
            r#"{"messages":[{"role":"user","content":"type mcp_servers web_fetch"}]}"#,
        ] {
            assert!(policy.prepare(post_body(body)).is_ok(), "{body}");
        }
        let count = ProxyRequest {
            body: Some(r#"{"tools":[{"name":"t","input_schema":{}}]}"#.into()),
            ..request("POST", "https://api.anthropic.com/v1/messages/count_tokens")
        };
        assert!(policy.prepare(count).is_ok());
    }

    #[test]
    fn refuses_server_tools_and_mcp_servers() {
        let policy = EgressPolicy::anthropic();
        for body in [
            r#"{"mcp_servers":[{"type":"url","url":"https://evil.example/mcp"}]}"#,
            r#"{"mcp_servers":null}"#,
            r#"{"mcp_servers":[]}"#,
            r#"{"mcp_servers":[]}"#,
            r#"{"tools":[{"type":"web_fetch_20250910","name":"web_fetch"}]}"#,
            r#"{"tools":[{"type":"web_search_20250305","name":"web_search"}]}"#,
            r#"{"tools":[{"type":"code_execution_20250825","name":"code_execution"}]}"#,
            r#"{"tools":[{"type":"bash_20250124","name":"bash"}]}"#,
            r#"{"tools":[{"type":"text_editor_20250728","name":"e"}]}"#,
            r#"{"tools":[{"type":"computer_20250124","name":"c"}]}"#,
            r#"{"tools":[{"type":"memory_20250818","name":"memory"}]}"#,
            r#"{"tools":[{"name":"ok","input_schema":{}},{"type":"web_fetch_x"}]}"#,
            r#"{"tools":[{"type":null,"name":"t"}]}"#,
            r#"{"tools":[{"type":1,"name":"t"}]}"#,
            r#"{"tools":[{"type":"Custom","name":"t"}]}"#,
            r#"{"tools":[{"type":"web_fetch_1"}]}"#,
            r#"{"tools":[{"type":"web_fetch_1","name":"t","type":"custom"}]}"#,
        ] {
            let found = policy.prepare(post_body(body)).unwrap_err();
            assert!(
                matches!(found, PolicyError::ServerTools | PolicyError::BadBody),
                "{body}: {found:?}"
            );
        }
        let count = ProxyRequest {
            body: Some(r#"{"tools":[{"type":"web_search_20250305"}]}"#.into()),
            ..request("POST", "https://api.anthropic.com/v1/messages/count_tokens")
        };
        assert_eq!(policy.prepare(count).unwrap_err(), PolicyError::ServerTools);
    }

    #[test]
    fn refuses_bodies_it_cannot_check() {
        let policy = EgressPolicy::anthropic();
        // Duplicate keys: the API might read a different copy than the policy did.
        for body in [
            "",
            "not json",
            "{",
            "[]",
            r#"[[{"type":"web_fetch_1"}]]"#,
            "null",
            r#""text""#,
            r#"{"tools":{"type":"web_fetch_1"}}"#,
            r#"{"tools":["web_fetch"]}"#,
            r#"{"tools":[],"tools":[{"type":"web_fetch_1"}]}"#,
            r#"{"tools":[{"type":"web_fetch_1"}],"tools":[]}"#,
            r#"{"mcp_servers":[],"mcp_servers":[]}"#,
            r#"{"model":"m"} trailing"#,
        ] {
            assert_eq!(
                policy.prepare(post_body(body)).unwrap_err(),
                PolicyError::BadBody,
                "{body}"
            );
        }
        let no_body = request("POST", "https://api.anthropic.com/v1/messages");
        assert_eq!(policy.prepare(no_body).unwrap_err(), PolicyError::BadBody);
    }

    #[test]
    fn checks_the_endpoint_before_parsing_the_body() {
        let policy = EgressPolicy::anthropic();
        let mut req = post_body("not json");
        req.url = "https://api.anthropic.com/v1/files".into();
        assert_eq!(policy.prepare(req).unwrap_err(), PolicyError::Destination);
    }

    #[test]
    fn rejects_relative_and_malformed_urls() {
        let policy = EgressPolicy::anthropic();
        for url in ["/v1/messages", "", "https://", "not a url"] {
            assert_eq!(
                policy.prepare(post(url)).unwrap_err(),
                PolicyError::BadUrl,
                "{url}"
            );
        }
    }

    #[test]
    fn rejects_methods_other_than_get_and_post() {
        let policy = EgressPolicy::anthropic();
        for method in ["PUT", "DELETE", "PATCH", "HEAD", "OPTIONS", "CONNECT", ""] {
            let found = policy.prepare(request(method, "https://api.anthropic.com/v1/messages"));
            assert_eq!(found.unwrap_err(), PolicyError::Method, "{method}");
        }
    }

    #[test]
    fn keeps_only_the_passed_headers_and_drops_credentials() {
        let mut req = post("https://api.anthropic.com/v1/messages");
        req.headers = [
            ("Anthropic-Version", "2023-06-01"),
            ("anthropic-beta", "a"),
            ("anthropic-beta", "b"),
            ("Content-Type", "application/json"),
            ("accept", "text/event-stream"),
            ("x-api-key", "test-key-from-js"),
            ("X-Api-Key", "test-key-from-js"),
            ("Authorization", "Bearer test-token"),
            ("cookie", "session=1"),
            ("host", "evil.example"),
            ("x-stainless-os", "Windows"),
        ]
        .into_iter()
        .map(|(n, v)| (n.to_owned(), v.to_owned()))
        .collect();

        let headers = EgressPolicy::anthropic().prepare(req).unwrap().headers;

        assert_eq!(headers.len(), 5);
        assert_eq!(headers["anthropic-version"], "2023-06-01");
        assert_eq!(
            headers.get_all("anthropic-beta").iter().collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(headers["content-type"], "application/json");
        assert_eq!(headers["accept"], "text/event-stream");
        for dropped in [
            "x-api-key",
            "authorization",
            "cookie",
            "host",
            "x-stainless-os",
        ] {
            assert!(!headers.contains_key(dropped), "{dropped}");
        }
    }

    #[test]
    fn rejects_a_passed_header_with_an_invalid_value() {
        let mut req = post("https://api.anthropic.com/v1/messages");
        req.headers = vec![(
            "anthropic-version".into(),
            "1\r\nx-api-key: injected".into(),
        )];
        assert_eq!(
            EgressPolicy::anthropic().prepare(req).unwrap_err(),
            PolicyError::BadHeader
        );
    }

    #[test]
    fn limits_the_body_to_32_mib() {
        let policy = EgressPolicy::anthropic();
        let mut req = post("https://api.anthropic.com/v1/messages");
        let json = |len: usize| format!(r#"{{"a":"{}"}}"#, "x".repeat(len - 8));
        req.body = Some(json(MAX_BODY_BYTES));
        assert!(policy.prepare(req.clone()).is_ok());
        req.body = Some(json(MAX_BODY_BYTES + 1));
        assert_eq!(policy.prepare(req).unwrap_err(), PolicyError::BodyTooLarge);
    }

    #[test]
    fn rejects_a_get_with_a_body() {
        let mut req = request("GET", "https://api.anthropic.com/v1/models");
        req.body = Some(String::new());
        assert_eq!(
            EgressPolicy::anthropic().prepare(req).unwrap_err(),
            PolicyError::BodyOnGet
        );
    }

    #[test]
    fn drops_the_fragment() {
        let url = EgressPolicy::anthropic()
            .prepare(post("https://api.anthropic.com/v1/messages#frag"))
            .unwrap()
            .url;
        assert_eq!(url.as_str(), "https://api.anthropic.com/v1/messages");
    }

    #[test]
    fn a_test_origin_allows_only_itself() {
        let policy = EgressPolicy::for_test_origin("http://127.0.0.1:4100");
        assert!(
            policy
                .prepare(post("http://127.0.0.1:4100/v1/messages"))
                .is_ok()
        );
        assert_eq!(
            policy
                .prepare(post("http://127.0.0.1:4101/v1/messages"))
                .unwrap_err(),
            PolicyError::Destination
        );
        assert_eq!(
            policy
                .prepare(post("https://api.anthropic.com/v1/messages"))
                .unwrap_err(),
            PolicyError::Destination
        );
    }

    #[test]
    fn an_unparsable_origin_fails_closed() {
        let policy = EgressPolicy::for_test_origin("not an origin");
        assert_eq!(
            policy
                .prepare(post("https://api.anthropic.com/v1/messages"))
                .unwrap_err(),
            PolicyError::Destination
        );
    }

    #[test]
    fn requests_deserialize_from_the_ipc_shape() {
        let json = r#"{"requestId":"r1","method":"POST","url":"https://api.anthropic.com/v1/messages",
            "headers":[["content-type","application/json"]],"body":"{}"}"#;
        let req: ProxyRequest = serde_json::from_str(json).unwrap();
        assert_eq!(
            req.headers,
            [("content-type".to_owned(), "application/json".to_owned())]
        );
        assert!(
            serde_json::from_str::<ProxyRequest>(
                r#"{"requestId":"r1","method":"GET","url":"u","headers":[],"body":null,"extra":1}"#
            )
            .is_err()
        );
    }
}
