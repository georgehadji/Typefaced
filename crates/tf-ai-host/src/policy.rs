//! `EgressPolicy`: decides whether a request from the webview may leave the machine, and
//! rebuilds it with only the allowed parts: one origin, four endpoints, client tools only,
//! a size limit, a header allowlist and an `anthropic-beta` value allowlist.

use reqwest::Method;
use reqwest::Url;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use serde::Deserialize;
use specta::Type;

use crate::body::check_body;

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

/// The `anthropic-beta` values that pass: only those the webview client sends
/// (`packages/ai`: `fallbacks: "default"`). One header may list several, comma-separated.
const ALLOWED_BETAS: [&str; 1] = ["server-side-fallback-2026-07-01"];

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
    #[error("an anthropic-beta value is not allowed; allowed values: {}", ALLOWED_BETAS.join(", "))]
    Beta,
    #[error("the request body has a top-level field the client does not send")]
    BodyField,
    #[error("the request body is larger than {} MiB", MAX_BODY_BYTES >> 20)]
    BodyTooLarge,
    #[error("a GET request cannot have a body")]
    BodyOnGet,
    #[error("the request body must be one JSON object without duplicate keys")]
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
        if !endpoint_allowed(&method, url.path()) || !query_allowed(&method, &url) {
            return Err(PolicyError::Destination);
        }
        let mut headers = passed_headers(&request.headers)?;
        // Only the two POST endpoints take a body; parsed only after the checks above.
        if method == Method::POST {
            check_body(request.body.as_deref())?;
            // The API must read the body as the JSON that was checked, nothing else.
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        }
        Ok(PreparedRequest {
            method,
            url,
            headers,
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

/// The query parameters the SDK sends: `beta=true` on every endpoint, and the paging
/// parameters on GET. No other parameter, no parameter twice, no empty `?`.
fn query_allowed(method: &Method, url: &Url) -> bool {
    let Some(query) = url.query() else {
        return true;
    };
    let mut seen = Vec::new();
    for pair in query.split('&') {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        let known = match name {
            "beta" => value == "true",
            "limit" | "before_id" | "after_id" => *method == Method::GET,
            _ => false,
        };
        if !known || seen.contains(&name) {
            return false;
        }
        seen.push(name);
    }
    true
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
        if *allowed == "anthropic-beta" && !beta_allowed(value) {
            return Err(PolicyError::Beta);
        }
        let value = HeaderValue::from_str(value).map_err(|_| PolicyError::BadHeader)?;
        headers.append(HeaderName::from_static(allowed), value);
    }
    Ok(headers)
}

/// Every comma-separated value is in [`ALLOWED_BETAS`] (exact match after trimming).
fn beta_allowed(value: &str) -> bool {
    value
        .split(',')
        .all(|beta| ALLOWED_BETAS.contains(&beta.trim_matches([' ', '\t'])))
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
            r#"{"mcp\u005fservers":[]}"#,
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
        let deep = format!("{{\"a\":{}1{}}}", "[".repeat(200), "]".repeat(200));
        assert_eq!(
            policy.prepare(post_body(&deep)).unwrap_err(),
            PolicyError::BadBody
        );
        let no_body = request("POST", "https://api.anthropic.com/v1/messages");
        assert_eq!(policy.prepare(no_body).unwrap_err(), PolicyError::BadBody);
    }

    #[test]
    fn refuses_content_the_api_would_fetch_from_a_url() {
        let policy = EgressPolicy::anthropic();
        let url_image =
            r#"{"type":"image","source":{"type":"url","url":"https://evil.example/x.png"}}"#;
        let url_document =
            r#"{"type":"document","source":{"type":"url","url":"https://evil.example/d.pdf"}}"#;
        for body in [
            format!(r#"{{"messages":[{{"role":"user","content":[{url_image}]}}]}}"#),
            format!(r#"{{"messages":[{{"role":"user","content":[{url_document}]}}]}}"#),
            format!(
                r#"{{"messages":[{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"t","content":[{url_image}]}}]}}]}}"#
            ),
            format!(r#"{{"system":[{url_document}]}}"#),
            r#"{"x":{"source":{"type":"file","file_id":"f"}}}"#.to_owned(),
            r#"{"x":{"source":{"type":null}}}"#.to_owned(),
            r#"{"x":{"source":{"url":"https://evil.example/"}}}"#.to_owned(),
            r#"{"x":{"source":{"type":"url"}}}"#.to_owned(),
            r#"{"x":{"source":{"type":"u\u0072l"}}}"#.to_owned(),
            format!(r#"{{"x":{{"source":{{"type":"{}"}}}}}}"#, "b".repeat(65)),
            r#"{"x":{"source":{"type":"content","content":[{"type":"image","source":{"type":"url","url":"u"}}]}}}"#.to_owned(),
            r#"{"x":{"source":["url"]}}"#.to_owned(),
            r#"{"x":{"source":1}}"#.to_owned(),
            r#"{"x":{"source":null}}"#.to_owned(),
        ] {
            assert_eq!(
                policy.prepare(post_body(&body)).unwrap_err(),
                PolicyError::ServerTools,
                "{body}"
            );
            let count = ProxyRequest {
                body: Some(body.clone()),
                ..request("POST", "https://api.anthropic.com/v1/messages/count_tokens")
            };
            assert_eq!(
                policy.prepare(count).unwrap_err(),
                PolicyError::ServerTools,
                "{body}"
            );
        }
    }

    #[test]
    fn allows_inline_content_sources() {
        let policy = EgressPolicy::anthropic();
        for body in [
            r#"{"messages":[{"role":"user","content":[{"type":"image","source":{"type":"base64","media_type":"image/png","data":"AA=="}}]}]}"#,
            r#"{"messages":[{"role":"user","content":[{"type":"document","source":{"type":"text","media_type":"text/plain","data":"hi"}}]}]}"#,
            r#"{"messages":[{"role":"user","content":[{"type":"document","source":{"type":"content","content":[{"type":"text","text":"hi"}]}}]}]}"#,
            r#"{"messages":[{"role":"user","content":[{"type":"search_result","source":"https://example.com","title":"t","content":[]}]}]}"#,
        ] {
            assert!(policy.prepare(post_body(body)).is_ok(), "{body}");
        }
    }

    #[test]
    fn refuses_duplicate_keys_anywhere_in_the_body() {
        let policy = EgressPolicy::anthropic();
        for body in [
            r#"{"model":"a","model":"b"}"#,
            r#"{"messages":[{"role":"user","role":"assistant"}]}"#,
            r#"{"a":{"b":{"c":1,"c":2}}}"#,
            r#"{"a":1,"a":2}"#,
            r#"{"x":{"source":{"type":"base64","type":"url"}}}"#,
        ] {
            assert_eq!(
                policy.prepare(post_body(body)).unwrap_err(),
                PolicyError::BadBody,
                "{body}"
            );
        }
    }

    #[test]
    fn a_post_is_always_sent_as_json() {
        let mut req = post_body("{}");
        req.headers = vec![(
            "content-type".into(),
            "application/x-www-form-urlencoded".into(),
        )];
        let headers = EgressPolicy::anthropic().prepare(req).unwrap().headers;
        assert_eq!(
            headers.get_all("content-type").iter().collect::<Vec<_>>(),
            ["application/json"]
        );
    }

    #[test]
    fn allows_only_the_known_query_parameters() {
        let policy = EgressPolicy::anthropic();
        let base = "https://api.anthropic.com";
        for url in [
            "/v1/messages",
            "/v1/messages?beta=true",
            "/v1/messages/count_tokens?beta=true",
        ] {
            assert!(
                policy.prepare(post(&format!("{base}{url}"))).is_ok(),
                "{url}"
            );
        }
        for url in [
            "/v1/models",
            "/v1/models?limit=20",
            "/v1/models?beta=true&limit=5&after_id=a&before_id=b",
            "/v1/models/claude-opus-5?beta=true",
        ] {
            assert!(
                policy
                    .prepare(request("GET", &format!("{base}{url}")))
                    .is_ok(),
                "{url}"
            );
        }
        for url in [
            "/v1/messages?beta=false",
            "/v1/messages?beta=true&x=1",
            "/v1/messages?",
            "/v1/messages?beta=true&beta=true",
            "/v1/messages?limit=5",
        ] {
            assert_eq!(
                policy.prepare(post(&format!("{base}{url}"))).unwrap_err(),
                PolicyError::Destination,
                "POST {url}"
            );
        }
        for url in [
            "/v1/models?x=1",
            "/v1/models?beta=false",
            "/v1/models?limit=1&limit=2",
        ] {
            assert_eq!(
                policy
                    .prepare(request("GET", &format!("{base}{url}")))
                    .unwrap_err(),
                PolicyError::Destination,
                "GET {url}"
            );
        }
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
            ("anthropic-beta", FALLBACK),
            ("anthropic-beta", " server-side-fallback-2026-07-01 "),
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
            [FALLBACK, " server-side-fallback-2026-07-01 "]
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

    const FALLBACK: &str = "server-side-fallback-2026-07-01";

    fn with_beta(values: &[&str]) -> ProxyRequest {
        let mut req = post("https://api.anthropic.com/v1/messages");
        req.headers = values
            .iter()
            .map(|v| ("Anthropic-Beta".to_owned(), (*v).to_owned()))
            .collect();
        req
    }

    #[test]
    fn allows_only_the_beta_the_client_sends() {
        let policy = EgressPolicy::anthropic();
        for values in [
            &[][..],
            &[FALLBACK][..],
            &[FALLBACK, FALLBACK][..],
            &["server-side-fallback-2026-07-01,server-side-fallback-2026-07-01"][..],
        ] {
            assert!(policy.prepare(with_beta(values)).is_ok(), "{values:?}");
        }
        let get = ProxyRequest {
            headers: vec![("anthropic-beta".into(), FALLBACK.into())],
            ..request("GET", "https://api.anthropic.com/v1/models")
        };
        assert!(policy.prepare(get).is_ok());
    }

    #[test]
    fn refuses_every_other_beta() {
        let policy = EgressPolicy::anthropic();
        for values in [
            &["mcp-client-2025-11-20"][..],
            &["code-execution-2025-08-25"][..],
            &["server-side-fallback-2026-06-01"][..],
            &["files-api-2025-04-14"][..],
            &["oauth-2025-04-20"][..],
            &["Server-Side-Fallback-2026-07-01"][..],
            &[FALLBACK, "mcp-client-2025-11-20"][..],
            &["server-side-fallback-2026-07-01,mcp-client-2025-11-20"][..],
            &["server-side-fallback-2026-07-01,"][..],
            &[""][..],
        ] {
            assert_eq!(
                policy.prepare(with_beta(values)).unwrap_err(),
                PolicyError::Beta,
                "{values:?}"
            );
        }
        let get = ProxyRequest {
            headers: vec![("anthropic-beta".into(), "mcp-client-2025-11-20".into())],
            ..request("GET", "https://api.anthropic.com/v1/models")
        };
        assert_eq!(policy.prepare(get).unwrap_err(), PolicyError::Beta);
    }

    #[test]
    fn trims_only_http_whitespace_around_a_beta() {
        let policy = EgressPolicy::anthropic();
        let spaced = format!(" {FALLBACK}\t,\t{FALLBACK} ");
        assert!(policy.prepare(with_beta(&[&spaced])).is_ok());
        for value in [
            " ".to_owned(),
            format!("{}{FALLBACK}", char::from(0xA0)),
            format!("{FALLBACK}{}", char::from(0x0B)),
        ] {
            assert_eq!(
                policy.prepare(with_beta(&[&value])).unwrap_err(),
                PolicyError::Beta,
                "{value:?}"
            );
        }
    }

    #[test]
    fn refuses_a_bad_beta_before_looking_at_the_body() {
        let mut req = post_body(r#"{"tool_choice":{"type":"auto"}}"#);
        req.headers = vec![("anthropic-beta".into(), "mcp-client-2025-11-20".into())];
        assert_eq!(
            EgressPolicy::anthropic().prepare(req).unwrap_err(),
            PolicyError::Beta
        );
    }

    #[test]
    fn allows_only_the_default_fallback() {
        let policy = EgressPolicy::anthropic();
        assert!(
            policy
                .prepare(post_body(r#"{"fallbacks":"default"}"#))
                .is_ok()
        );
        for body in [
            r#"{"fallbacks":[{"model":"claude-opus-4-8"}]}"#,
            r#"{"fallbacks":[{"model":"m","tools":[{"type":"web_search_20250305"}]}]}"#,
            r#"{"fallbacks":"Default"}"#,
            r#"{"fallbacks":null}"#,
            r#"{"fallbacks":{}}"#,
        ] {
            assert_eq!(
                policy.prepare(post_body(body)).unwrap_err(),
                PolicyError::BodyField,
                "{body}"
            );
        }
    }

    #[test]
    fn allows_the_top_level_fields_the_client_sends() {
        let policy = EgressPolicy::anthropic();
        let body = r#"{"model":"claude-opus-5-5","max_tokens":64000,
            "messages":[{"role":"user","content":"hi"}],"system":"s",
            "tools":[{"type":"custom","name":"t","input_schema":{},"eager_input_streaming":true}],
            "thinking":{"type":"adaptive"},"output_config":{"effort":"medium"},
            "stream":true,"fallbacks":"default"}"#;
        assert!(policy.prepare(post_body(body)).is_ok());
        let count = ProxyRequest {
            body: Some(r#"{"model":"m","messages":[],"tools":[],"system":"s"}"#.into()),
            ..request("POST", "https://api.anthropic.com/v1/messages/count_tokens")
        };
        assert!(policy.prepare(count).is_ok());
    }

    #[test]
    fn refuses_every_other_top_level_field() {
        let policy = EgressPolicy::anthropic();
        for field in [
            "tool_choice",
            "metadata",
            "temperature",
            "top_p",
            "top_k",
            "stop_sequences",
            "container",
            "context_management",
            "speed",
            "service_tier",
            "inference_geo",
            "Model",
            "x",
            "",
        ] {
            let body = format!(r#"{{"model":"m","{field}":1}}"#);
            assert_eq!(
                policy.prepare(post_body(&body)).unwrap_err(),
                PolicyError::BodyField,
                "{field}"
            );
            let count = ProxyRequest {
                body: Some(body),
                ..request("POST", "https://api.anthropic.com/v1/messages/count_tokens")
            };
            assert_eq!(
                policy.prepare(count).unwrap_err(),
                PolicyError::BodyField,
                "{field}"
            );
        }
        // Field names are matched with JSON escapes decoded, as the API reads them.
        assert_eq!(
            policy
                .prepare(post_body(r#"{"tool\u005fchoice":{"type":"auto"}}"#))
                .unwrap_err(),
            PolicyError::BodyField
        );
        assert!(policy.prepare(post_body(r#"{"m\u006fdel":"m"}"#)).is_ok());
        assert_eq!(
            policy
                .prepare(post_body(r#"{"m\u006fdel":"a","model":"b"}"#))
                .unwrap_err(),
            PolicyError::BadBody
        );
        // Nested objects keep their own fields: only the top level is checked here.
        assert!(
            policy
                .prepare(post_body(
                    r#"{"messages":[{"role":"user","content":"x","metadata":1}]}"#
                ))
                .is_ok()
        );
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
        let json = |len: usize| format!(r#"{{"system":"{}"}}"#, "x".repeat(len - 13));
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
