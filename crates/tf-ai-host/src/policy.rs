//! `EgressPolicy`: decides whether a request from the webview may leave the machine, and
//! rebuilds it with only the allowed parts: one origin (OpenRouter), one endpoint
//! (`POST /api/v1/messages`), client tools only, a size limit and a header allowlist.

use reqwest::Method;
use reqwest::Url;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use serde::Deserialize;
use specta::Type;

use crate::body::check_body;

/// The only destination: OpenRouter, through its Anthropic-compatible Messages API.
pub const OPENROUTER_ORIGIN: &str = "https://openrouter.ai";

/// The only endpoint the client calls (the SDK's `baseURL` is `{origin}/api`).
const MESSAGES_PATH: &str = "/api/v1/messages";

/// Upper bound for a request body: 32 MiB.
pub const MAX_BODY_BYTES: usize = 32 * 1024 * 1024;

/// Request headers that pass through. Every other header is dropped, including
/// `x-api-key`, `authorization` and `cookie`; the host adds `authorization` itself.
const PASSED_HEADERS: [&str; 3] = ["anthropic-version", "content-type", "accept"];

/// A request as the webview's `fetch` describes it, before any check.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProxyRequest {
    /// Chosen by the caller (e.g. a UUID); `ai_abort` cancels the request by this ID.
    pub request_id: String,
    /// `POST`; any other method is refused.
    pub method: String,
    /// Absolute URL, e.g. `https://openrouter.ai/api/v1/messages`.
    pub url: String,
    /// Header name and value pairs.
    pub headers: Vec<(String, String)>,
    /// UTF-8 body, if any (the Messages API takes JSON).
    pub body: Option<String>,
}

/// A request that passed the policy: ready to send once the key is added.
#[derive(Debug)]
pub(crate) struct PreparedRequest {
    /// The requested model (checked), for the usage ledger.
    pub model: String,
    pub method: Method,
    pub url: Url,
    pub headers: HeaderMap,
    pub body: Option<String>,
}

/// Why the policy refused a request. The messages never echo header values or bodies.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PolicyError {
    #[error("only POST requests are forwarded")]
    Method,
    #[error("the URL is not a valid absolute URL")]
    BadUrl,
    #[error("the destination is not allowed: only POST {MESSAGES_PATH} on {OPENROUTER_ORIGIN} is")]
    Destination,
    #[error("a forwarded header has an invalid name or value")]
    BadHeader,
    #[error("anthropic-beta headers are not forwarded")]
    Beta,
    #[error("the request body has a field the client does not send")]
    BodyField,
    #[error(
        "a model or fallback model is not allowed: only anthropic/<model> IDs without a \
         :variant, with at most 3 fallbacks"
    )]
    Model,
    #[error("max_tokens or thinking is outside what the client sends")]
    Setting,
    #[error("the request body is larger than {} MiB", MAX_BODY_BYTES >> 20)]
    BodyTooLarge,
    #[error("the request body must be one JSON object without duplicate keys")]
    BadBody,
    #[error("server-side tools and MCP servers are not allowed; only client tools are")]
    ServerTools,
    #[error("content the API would fetch from elsewhere (a url or file source) is not allowed")]
    FetchedSource,
}

/// The egress rules: one allowed origin and the endpoint [`MESSAGES_PATH`].
#[derive(Debug, Clone)]
pub struct EgressPolicy {
    origin: Option<Url>,
}

impl EgressPolicy {
    /// The production policy: only OpenRouter at `https://openrouter.ai`.
    pub fn openrouter() -> Self {
        Self::for_origin(OPENROUTER_ORIGIN)
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
        if !request.method.eq_ignore_ascii_case("POST") {
            return Err(PolicyError::Method);
        }
        let url = self.allowed_url(&request.url)?;
        if request
            .body
            .as_ref()
            .is_some_and(|b| b.len() > MAX_BODY_BYTES)
        {
            return Err(PolicyError::BodyTooLarge);
        }
        if url.path() != MESSAGES_PATH || !query_allowed(&url) {
            return Err(PolicyError::Destination);
        }
        let mut headers = passed_headers(&request.headers)?;
        // Parsed only after the checks above.
        let model = check_body(request.body.as_deref())?;
        // The API must read the body as the JSON that was checked, nothing else.
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        Ok(PreparedRequest {
            model,
            method: Method::POST,
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

/// The SDK's beta Messages call adds `?beta=true`; no other query, no empty `?`.
fn query_allowed(url: &Url) -> bool {
    url.query().is_none_or(|query| query == "beta=true")
}

fn passed_headers(raw: &[(String, String)]) -> Result<HeaderMap, PolicyError> {
    let mut headers = HeaderMap::new();
    for (name, value) in raw {
        // The client sends no beta; refused, not dropped, so a new one is noticed.
        if name.eq_ignore_ascii_case("anthropic-beta") {
            return Err(PolicyError::Beta);
        }
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

    const MESSAGES: &str = "https://openrouter.ai/api/v1/messages";
    const MODEL: &str = "anthropic/claude-opus-5.5";

    fn request(method: &str, url: &str) -> ProxyRequest {
        ProxyRequest {
            request_id: "r1".into(),
            method: method.into(),
            url: url.into(),
            headers: Vec::new(),
            body: None,
        }
    }

    /// A POST with the smallest body the policy accepts.
    fn post(url: &str) -> ProxyRequest {
        ProxyRequest {
            body: Some(format!(r#"{{"model":"{MODEL}"}}"#)),
            ..request("POST", url)
        }
    }

    fn post_body(body: &str) -> ProxyRequest {
        ProxyRequest {
            body: Some(body.into()),
            ..request("POST", MESSAGES)
        }
    }

    /// `fields` (JSON members, without braces) after a valid `model`.
    fn with_model(fields: &str) -> String {
        if fields.is_empty() {
            format!(r#"{{"model":"{MODEL}"}}"#)
        } else {
            format!(r#"{{"model":"{MODEL}",{fields}}}"#)
        }
    }

    #[test]
    fn allows_a_post_to_the_openrouter_messages_endpoint() {
        let policy = EgressPolicy::openrouter();
        let prepared = policy
            .prepare(post(&format!("{MESSAGES}?beta=true")))
            .unwrap();
        assert_eq!(prepared.method, Method::POST);
        assert_eq!(
            prepared.url.as_str(),
            "https://openrouter.ai/api/v1/messages?beta=true"
        );
        assert_eq!(prepared.body, Some(with_model("")));
        assert_eq!(prepared.model, MODEL);

        let normalised = policy
            .prepare(post("https://OpenRouter.AI:443/api/v1/./messages"))
            .unwrap();
        assert_eq!(normalised.url.as_str(), MESSAGES);
        let lower = policy.prepare(ProxyRequest {
            method: "post".into(),
            ..post(MESSAGES)
        });
        assert!(lower.is_ok());
    }

    #[test]
    fn rejects_every_other_destination() {
        let policy = EgressPolicy::openrouter();
        for url in [
            "http://openrouter.ai/api/v1/messages",
            "https://openrouter.ai:8443/api/v1/messages",
            "https://openrouter.ai./api/v1/messages",
            "https://evil.example/api/v1/messages",
            "https://openrouter.ai.evil.com/api/v1/messages",
            "https://openrouter.ai@evil.com/api/v1/messages",
            "https://evil.com@openrouter.ai/api/v1/messages",
            "https://user:pass@openrouter.ai/api/v1/messages",
            "https://www.openrouter.ai/api/v1/messages",
            "https://api.openrouter.ai/api/v1/messages",
            "https://api.anthropic.com/v1/messages",
            "https://api.anthropic.com/api/v1/messages",
            "https://openrouter.ai/v1/messages",
            "https://openrouter.ai/api/v2/messages",
            "https://openrouter.ai/api/v1",
            "https://openrouter.ai/",
            "https://openrouter.ai/api/v1/../admin",
            "https://openrouter.ai/api/v1/%2e%2e/admin",
            "https://openrouter.ai/api/v1/..\\admin",
            "https://openrouter.ai/api/v1/messages/../keys",
        ] {
            assert_eq!(
                policy.prepare(post(url)).unwrap_err(),
                PolicyError::Destination,
                "{url}"
            );
        }
    }

    #[test]
    fn refuses_every_other_endpoint() {
        let policy = EgressPolicy::openrouter();
        let base = "https://openrouter.ai";
        for path in [
            "/api/v1/chat/completions",
            "/api/v1/responses",
            "/api/v1/messages/count_tokens",
            "/api/v1/messages/batches",
            "/api/v1/messages/",
            "/api/v1/Messages",
            "/api/v1/models",
            "/api/v1/keys",
            "/api/v1/credits",
            "/api/v1/presets/p/messages",
            "/api/v1/messages%2F",
            "/api/v1/messages/%2e%2e/keys",
        ] {
            assert_eq!(
                policy.prepare(post(&format!("{base}{path}"))).unwrap_err(),
                PolicyError::Destination,
                "POST {path}"
            );
        }
    }

    #[test]
    fn allows_only_post() {
        let policy = EgressPolicy::openrouter();
        for method in [
            "GET", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS", "CONNECT", "",
        ] {
            let found = policy.prepare(request(method, MESSAGES));
            assert_eq!(found.unwrap_err(), PolicyError::Method, "{method}");
        }
        let get_models = request("GET", "https://openrouter.ai/api/v1/models");
        assert_eq!(policy.prepare(get_models).unwrap_err(), PolicyError::Method);
    }

    #[test]
    fn allows_client_tools_and_custom_tools() {
        let policy = EgressPolicy::openrouter();
        for fields in [
            r#""messages":[]"#,
            r#""tools":[]"#,
            r#""tools":[{"name":"get_font_summary","input_schema":{"type":"object"}}]"#,
            r#""tools":[{"type":"custom","name":"t","input_schema":{}}]"#,
            r#""messages":[{"role":"user","content":"type mcp_servers web_fetch plugins"}]"#,
        ] {
            let body = with_model(fields);
            assert!(policy.prepare(post_body(&body)).is_ok(), "{body}");
        }
    }

    #[test]
    fn refuses_server_tools_and_mcp_servers() {
        let policy = EgressPolicy::openrouter();
        for fields in [
            r#""mcp_servers":[{"type":"url","url":"https://evil.example/mcp"}]"#,
            r#""mcp_servers":null"#,
            r#""mcp_servers":[]"#,
            r#""mcp\u005fservers":[]"#,
            r#""tools":[{"type":"web_fetch_20250910","name":"web_fetch"}]"#,
            r#""tools":[{"type":"web_search_20250305","name":"web_search"}]"#,
            r#""tools":[{"type":"code_execution_20250825","name":"code_execution"}]"#,
            r#""tools":[{"type":"bash_20250124","name":"bash"}]"#,
            r#""tools":[{"type":"text_editor_20250728","name":"e"}]"#,
            r#""tools":[{"type":"computer_20250124","name":"c"}]"#,
            r#""tools":[{"type":"memory_20250818","name":"memory"}]"#,
            r#""tools":[{"type":"advisor_20260301","name":"a"}]"#,
            r#""tools":[{"type":"openrouter:web_search"}]"#,
            r#""tools":[{"type":"openrouter:web_fetch"}]"#,
            r#""tools":[{"type":"openrouter:bash"}]"#,
            r#""tools":[{"type":"openrouter:shell"}]"#,
            r#""tools":[{"type":"openrouter:image_generation"}]"#,
            r#""tools":[{"name":"ok","input_schema":{}},{"type":"web_fetch_x"}]"#,
            r#""tools":[{"type":null,"name":"t"}]"#,
            r#""tools":[{"type":1,"name":"t"}]"#,
            r#""tools":[{"type":"Custom","name":"t"}]"#,
            r#""tools":[{"type":"web_fetch_1"}]"#,
        ] {
            let body = with_model(fields);
            assert_eq!(
                policy.prepare(post_body(&body)).unwrap_err(),
                PolicyError::ServerTools,
                "{body}"
            );
        }
    }

    #[test]
    fn refuses_bodies_it_cannot_check() {
        let policy = EgressPolicy::openrouter();
        // Duplicate keys: the API might read a different copy than the policy did.
        for body in [
            "",
            "not json",
            "{",
            "[]",
            r#"[[{"type":"web_fetch_1"}]]"#,
            "null",
            r#""text""#,
            r#"{"model":"anthropic/m","tools":{"type":"web_fetch_1"}}"#,
            r#"{"model":"anthropic/m","tools":["web_fetch"]}"#,
            r#"{"tools":[],"tools":[{"type":"web_fetch_1"}]}"#,
            r#"{"tools":[{"type":"web_fetch_1"}],"tools":[]}"#,
            r#"{"mcp_servers":[],"mcp_servers":[]}"#,
            r#"{"model":"anthropic/m"} trailing"#,
            r#"{"model":1}"#,
            r#"{"model":null}"#,
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
        let no_body = request("POST", MESSAGES);
        assert_eq!(policy.prepare(no_body).unwrap_err(), PolicyError::BadBody);
    }

    #[test]
    fn refuses_content_the_api_would_fetch_from_a_url() {
        let policy = EgressPolicy::openrouter();
        let url_image =
            r#"{"type":"image","source":{"type":"url","url":"https://evil.example/x.png"}}"#;
        let url_document =
            r#"{"type":"document","source":{"type":"url","url":"https://evil.example/d.pdf"}}"#;
        for fields in [
            format!(r#""messages":[{{"role":"user","content":[{url_image}]}}]"#),
            format!(r#""messages":[{{"role":"user","content":[{url_document}]}}]"#),
            format!(
                r#""messages":[{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"t","content":[{url_image}]}}]}}]"#
            ),
            format!(r#""system":[{url_document}]"#),
            r#""x":{"source":{"type":"file","file_id":"f"}}"#.to_owned(),
            r#""x":{"source":{"type":null}}"#.to_owned(),
            r#""x":{"source":{"url":"https://evil.example/"}}"#.to_owned(),
            r#""x":{"source":{"type":"url"}}"#.to_owned(),
            r#""x":{"source":{"type":"u\u0072l"}}"#.to_owned(),
            format!(r#""x":{{"source":{{"type":"{}"}}}}"#, "b".repeat(65)),
            r#""x":{"source":{"type":"content","content":[{"type":"image","source":{"type":"url","url":"u"}}]}}"#.to_owned(),
            r#""x":{"source":["url"]}"#.to_owned(),
            r#""x":{"source":1}"#.to_owned(),
            r#""x":{"source":null}"#.to_owned(),
        ] {
            let body = with_model(&fields);
            assert_eq!(
                policy.prepare(post_body(&body)).unwrap_err(),
                PolicyError::FetchedSource,
                "{body}"
            );
        }
    }

    #[test]
    fn allows_inline_content_sources() {
        let policy = EgressPolicy::openrouter();
        for fields in [
            r#""messages":[{"role":"user","content":[{"type":"image","source":{"type":"base64","media_type":"image/png","data":"AA=="}}]}]"#,
            r#""messages":[{"role":"user","content":[{"type":"document","source":{"type":"text","media_type":"text/plain","data":"hi"}}]}]"#,
            r#""messages":[{"role":"user","content":[{"type":"document","source":{"type":"content","content":[{"type":"text","text":"hi"}]}}]}]"#,
            r#""messages":[{"role":"user","content":[{"type":"search_result","source":"https://example.com","title":"t","content":[]}]}]"#,
        ] {
            let body = with_model(fields);
            assert!(policy.prepare(post_body(&body)).is_ok(), "{body}");
        }
    }

    #[test]
    fn refuses_duplicate_keys_anywhere_in_the_body() {
        let policy = EgressPolicy::openrouter();
        for body in [
            r#"{"model":"anthropic/a","model":"anthropic/b"}"#,
            r#"{"model":"anthropic/a","messages":[{"role":"user","role":"assistant"}]}"#,
            r#"{"model":"anthropic/a","a":{"b":{"c":1,"c":2}}}"#,
            r#"{"a":1,"a":2}"#,
            r#"{"x":{"source":{"type":"base64","type":"url"}}}"#,
            r#"{"tools":[{"type":"web_fetch_1","name":"t","type":"custom"}]}"#,
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
        let mut req = post(MESSAGES);
        req.headers = vec![(
            "content-type".into(),
            "application/x-www-form-urlencoded".into(),
        )];
        let headers = EgressPolicy::openrouter().prepare(req).unwrap().headers;
        assert_eq!(
            headers.get_all("content-type").iter().collect::<Vec<_>>(),
            ["application/json"]
        );
    }

    #[test]
    fn allows_only_the_beta_query_parameter() {
        let policy = EgressPolicy::openrouter();
        for url in [MESSAGES.to_owned(), format!("{MESSAGES}?beta=true")] {
            assert!(policy.prepare(post(&url)).is_ok(), "{url}");
        }
        for query in [
            "?beta=false",
            "?beta=true&x=1",
            "?",
            "?beta=true&beta=true",
            "?limit=5",
            "?beta=TRUE",
        ] {
            assert_eq!(
                policy
                    .prepare(post(&format!("{MESSAGES}{query}")))
                    .unwrap_err(),
                PolicyError::Destination,
                "{query}"
            );
        }
    }

    #[test]
    fn checks_the_endpoint_before_parsing_the_body() {
        let policy = EgressPolicy::openrouter();
        let mut req = post_body("not json");
        req.url = "https://openrouter.ai/api/v1/keys".into();
        assert_eq!(policy.prepare(req).unwrap_err(), PolicyError::Destination);
    }

    #[test]
    fn rejects_relative_and_malformed_urls() {
        let policy = EgressPolicy::openrouter();
        for url in ["/api/v1/messages", "", "https://", "not a url"] {
            assert_eq!(
                policy.prepare(post(url)).unwrap_err(),
                PolicyError::BadUrl,
                "{url}"
            );
        }
    }

    #[test]
    fn keeps_only_the_passed_headers_and_drops_credentials() {
        let mut req = post(MESSAGES);
        req.headers = [
            ("Anthropic-Version", "2023-06-01"),
            ("Content-Type", "application/json"),
            ("accept", "text/event-stream"),
            ("x-api-key", "test-key-from-js"),
            ("X-Api-Key", "test-key-from-js"),
            ("Authorization", "Bearer test-token"),
            ("cookie", "session=1"),
            ("host", "evil.example"),
            ("x-stainless-os", "Windows"),
            ("HTTP-Referer", "https://evil.example/"),
            ("X-Title", "spoofed"),
        ]
        .into_iter()
        .map(|(n, v)| (n.to_owned(), v.to_owned()))
        .collect();

        let headers = EgressPolicy::openrouter().prepare(req).unwrap().headers;

        assert_eq!(headers.len(), 3);
        assert_eq!(headers["anthropic-version"], "2023-06-01");
        assert_eq!(headers["content-type"], "application/json");
        assert_eq!(headers["accept"], "text/event-stream");
        for dropped in [
            "x-api-key",
            "authorization",
            "cookie",
            "host",
            "x-stainless-os",
            "http-referer",
            "x-title",
        ] {
            assert!(!headers.contains_key(dropped), "{dropped}");
        }
    }

    #[test]
    fn refuses_any_anthropic_beta_header() {
        let policy = EgressPolicy::openrouter();
        for value in [
            "server-side-fallback-2026-07-01",
            "mcp-client-2025-11-20",
            "files-api-2025-04-14",
            "",
        ] {
            let mut req = post(MESSAGES);
            req.headers = vec![("Anthropic-Beta".into(), value.into())];
            assert_eq!(
                policy.prepare(req).unwrap_err(),
                PolicyError::Beta,
                "{value:?}"
            );
        }
        // Checked before the body is looked at.
        let mut req = post_body(r#"{"plugins":[{"id":"web"}]}"#);
        req.headers = vec![("anthropic-beta".into(), "x".into())];
        assert_eq!(policy.prepare(req).unwrap_err(), PolicyError::Beta);
    }

    #[test]
    fn allows_the_top_level_fields_the_client_sends() {
        let policy = EgressPolicy::openrouter();
        let body = r#"{"model":"anthropic/claude-opus-5.5","max_tokens":64000,
            "messages":[{"role":"user","content":"hi"}],"system":"s",
            "tools":[{"name":"t","input_schema":{}}],
            "thinking":{"type":"adaptive"},"output_config":{"effort":"medium"},
            "stream":true,
            "fallbacks":[{"model":"anthropic/claude-opus-5"},{"model":"anthropic/claude-sonnet-5.5"}]}"#;
        assert!(policy.prepare(post_body(body)).is_ok());
    }

    #[test]
    fn refuses_every_other_top_level_field() {
        let policy = EgressPolicy::openrouter();
        for field in [
            // OpenRouter extensions: routing, plugins (the web plugin fetches the web),
            // tracking and data-handling switches.
            "plugins",
            "provider",
            "models",
            "route",
            "session_id",
            "trace",
            "safeguards",
            "metadata",
            "user",
            "stop_server_tools_when",
            "service_tier",
            "speed",
            // Messages API fields the client does not send.
            "tool_choice",
            "temperature",
            "top_p",
            "top_k",
            "stop_sequences",
            "cache_control",
            "context_management",
            "container",
            "inference_geo",
            "Model",
            "x",
            "",
        ] {
            let body = with_model(&format!(r#""{field}":1"#));
            assert_eq!(
                policy.prepare(post_body(&body)).unwrap_err(),
                PolicyError::BodyField,
                "{field}"
            );
        }
        // Field names are matched with JSON escapes decoded, as the API reads them.
        let escaped = with_model(r#""plug\u0069ns":[{"id":"web"}]"#);
        assert_eq!(
            policy.prepare(post_body(&escaped)).unwrap_err(),
            PolicyError::BodyField
        );
        assert!(
            policy
                .prepare(post_body(r#"{"m\u006fdel":"anthropic/m"}"#))
                .is_ok()
        );
        assert_eq!(
            policy
                .prepare(post_body(
                    r#"{"m\u006fdel":"anthropic/a","model":"anthropic/b"}"#
                ))
                .unwrap_err(),
            PolicyError::BadBody
        );
        // Nested objects keep their own fields: only the top level is checked here.
        let nested = with_model(r#""messages":[{"role":"user","content":"x","metadata":1}]"#);
        assert!(policy.prepare(post_body(&nested)).is_ok());
    }

    #[test]
    fn requires_one_vendor_and_model_id() {
        let policy = EgressPolicy::openrouter();
        for model in [
            "anthropic/claude-opus-5.5",
            "anthropic/claude-sonnet-4.6",
            "anthropic/claude_x-1",
        ] {
            let body = format!(r#"{{"model":"{model}"}}"#);
            assert!(policy.prepare(post_body(&body)).is_ok(), "{model}");
        }
        let long = format!("anthropic/{}", "a".repeat(120));
        for model in [
            "",
            "claude-opus-5",
            "anthropic/",
            "/claude",
            "anthropic/claude/x",
            "Anthropic/claude",
            "anthropic/Claude",
            "anthropic/claude opus",
            "anthropic/claude\u{0}",
            "anthropic//claude",
            "../claude",
            // OpenRouter's routers pick models (and providers) themselves.
            "openrouter/auto",
            "openrouter/pareto-code",
            // Only Claude models are routed to.
            "meta-llama/llama-3.1-8b",
            "openai/gpt-5",
            "anthropic.evil/claude",
            // Variants switch on refused features: the web plugin, provider sorting,
            // free endpoints that may train on prompts.
            "anthropic/claude-opus-5.5:online",
            "anthropic/claude-opus-5.5:nitro",
            "anthropic/claude-opus-5.5:floor",
            "anthropic/claude-opus-5.5:free",
            "anthropic/claude-haiku-4.5:batch",
            long.as_str(),
        ] {
            let body = serde_json::json!({ "model": model }).to_string();
            assert_eq!(
                policy.prepare(post_body(&body)).unwrap_err(),
                PolicyError::Model,
                "{model:?}"
            );
        }
        // Without a model, OpenRouter would pick the account's default.
        assert_eq!(
            policy.prepare(post_body(r#"{"messages":[]}"#)).unwrap_err(),
            PolicyError::Model
        );
    }

    #[test]
    fn allows_up_to_three_fallback_models_in_the_openrouter_shape() {
        let policy = EgressPolicy::openrouter();
        for fallbacks in [
            "[]",
            r#"[{"model":"anthropic/claude-opus-5"}]"#,
            r#"[{"model":"anthropic/a"},{"model":"anthropic/b"},{"model":"anthropic/c"}]"#,
        ] {
            let body = with_model(&format!(r#""fallbacks":{fallbacks}"#));
            assert!(policy.prepare(post_body(&body)).is_ok(), "{fallbacks}");
        }
        for fallbacks in [
            r#"[{"model":"anthropic/a"},{"model":"anthropic/b"},{"model":"anthropic/c"},{"model":"anthropic/d"}]"#,
            r#"[{"model":"openrouter/auto"}]"#,
            r#"[{"model":"claude-opus-5"}]"#,
            r#"[{"model":"anthropic/claude-opus-5:online"}]"#,
            r#"[{"model":"openai/gpt-5"}]"#,
        ] {
            let body = with_model(&format!(r#""fallbacks":{fallbacks}"#));
            assert_eq!(
                policy.prepare(post_body(&body)).unwrap_err(),
                PolicyError::Model,
                "{fallbacks}"
            );
        }
        // Per-attempt overrides could change tools or providers; only `model` passes.
        for fallbacks in [
            r#"[{"model":"anthropic/a","tools":[{"type":"web_search_20250305"}]}]"#,
            r#"[{"model":"anthropic/a","provider":{"data_collection":"allow"}}]"#,
            r#"[{"model":"anthropic/a","max_tokens":1}]"#,
        ] {
            let body = with_model(&format!(r#""fallbacks":{fallbacks}"#));
            assert_eq!(
                policy.prepare(post_body(&body)).unwrap_err(),
                PolicyError::BodyField,
                "{fallbacks}"
            );
        }
        // Anthropic's server-side `"default"` and other shapes are refused.
        for fallbacks in [r#""default""#, "{}", r#"["anthropic/a"]"#, r#"[{}]"#] {
            let body = with_model(&format!(r#""fallbacks":{fallbacks}"#));
            assert_eq!(
                policy.prepare(post_body(&body)).unwrap_err(),
                PolicyError::BadBody,
                "{fallbacks}"
            );
        }
    }

    #[test]
    fn allows_only_the_clients_max_tokens_and_thinking() {
        let policy = EgressPolicy::openrouter();
        for fields in [
            r#""max_tokens":64000"#,
            r#""max_tokens":1"#,
            r#""thinking":{"type":"adaptive"}"#,
        ] {
            let body = with_model(fields);
            assert!(policy.prepare(post_body(&body)).is_ok(), "{body}");
        }
        for fields in [
            r#""max_tokens":64001"#,
            r#""thinking":{"type":"enabled","budget_tokens":100000}"#,
            r#""thinking":{"type":"adaptive","display":"summarized"}"#,
            r#""thinking":{"type":"disabled"}"#,
            r#""thinking":null"#,
        ] {
            let body = with_model(fields);
            assert_eq!(
                policy.prepare(post_body(&body)).unwrap_err(),
                PolicyError::Setting,
                "{body}"
            );
        }
        for fields in [
            r#""max_tokens":-1"#,
            r#""max_tokens":"1""#,
            r#""max_tokens":1.5"#,
        ] {
            let body = with_model(fields);
            assert_eq!(
                policy.prepare(post_body(&body)).unwrap_err(),
                PolicyError::BadBody,
                "{body}"
            );
        }
    }

    #[test]
    fn rejects_a_passed_header_with_an_invalid_value() {
        let mut req = post(MESSAGES);
        req.headers = vec![(
            "anthropic-version".into(),
            "1\r\nx-api-key: injected".into(),
        )];
        assert_eq!(
            EgressPolicy::openrouter().prepare(req).unwrap_err(),
            PolicyError::BadHeader
        );
    }

    #[test]
    fn limits_the_body_to_32_mib() {
        let policy = EgressPolicy::openrouter();
        let mut req = post(MESSAGES);
        let prefix = format!(r#"{{"model":"{MODEL}","system":""#);
        let json = |len: usize| format!(r#"{prefix}{}"}}"#, "x".repeat(len - prefix.len() - 2));
        req.body = Some(json(MAX_BODY_BYTES));
        assert_eq!(req.body.as_ref().unwrap().len(), MAX_BODY_BYTES);
        assert!(policy.prepare(req.clone()).is_ok());
        req.body = Some(json(MAX_BODY_BYTES + 1));
        assert_eq!(policy.prepare(req).unwrap_err(), PolicyError::BodyTooLarge);
    }

    #[test]
    fn drops_the_fragment() {
        let url = EgressPolicy::openrouter()
            .prepare(post(&format!("{MESSAGES}#frag")))
            .unwrap()
            .url;
        assert_eq!(url.as_str(), MESSAGES);
    }

    #[test]
    fn a_test_origin_allows_only_itself() {
        let policy = EgressPolicy::for_test_origin("http://127.0.0.1:4100");
        assert!(
            policy
                .prepare(post("http://127.0.0.1:4100/api/v1/messages"))
                .is_ok()
        );
        for url in ["http://127.0.0.1:4101/api/v1/messages", MESSAGES] {
            assert_eq!(
                policy.prepare(post(url)).unwrap_err(),
                PolicyError::Destination,
                "{url}"
            );
        }
    }

    #[test]
    fn an_unparsable_origin_fails_closed() {
        let policy = EgressPolicy::for_test_origin("not an origin");
        assert_eq!(
            policy.prepare(post(MESSAGES)).unwrap_err(),
            PolicyError::Destination
        );
    }

    #[test]
    fn requests_deserialize_from_the_ipc_shape() {
        let json = r#"{"requestId":"r1","method":"POST","url":"https://openrouter.ai/api/v1/messages",
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
