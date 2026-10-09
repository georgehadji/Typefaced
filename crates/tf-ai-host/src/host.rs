//! `EgressHost`: forwards a request that passed the [`EgressPolicy`] to OpenRouter
//! with the key from the [`CredentialVault`], and streams the response back as events:
//! the head first, then text chunks, then an end, error or aborted marker.

use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use reqwest::Response;
use reqwest::header::{AUTHORIZATION, HeaderValue, SET_COOKIE};
use serde::Serialize;
use specta::Type;
use tokio::sync::oneshot;

use crate::ledger::{UsageLedger, UsageRecord, shorten};
use crate::policy::{EgressPolicy, ProxyRequest};
use crate::vault::CredentialVault;

/// Time allowed to open the TCP and TLS connection.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Longest silence between two reads. A non-streaming request may wait minutes for its
/// first byte (the SDK's own default timeout is 10 minutes); streams send pings.
const READ_TIMEOUT: Duration = Duration::from_secs(600);
/// Longest request ID, in bytes.
const MAX_REQUEST_ID_BYTES: usize = 64;
/// Most requests in flight at once; more are refused, so a runaway caller cannot open
/// unbounded connections.
const MAX_IN_FLIGHT: usize = 16;

/// One message on the response channel, in this order: `Head`, any number of `Chunk`s,
/// then exactly one of `End`, `Error` or `Aborted`. A refused request sends only `Error`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProxyEvent {
    /// Status and response headers (without `set-cookie`). The caller's `fetch` can
    /// resolve on this message, before the body arrives.
    Head {
        status: u16,
        headers: Vec<(String, String)>,
    },
    /// The next part of the body as UTF-8 text. A character split across network reads
    /// is sent whole in the later chunk; invalid bytes become U+FFFD.
    Chunk { text: String },
    /// The body ended normally.
    End,
    /// The request was refused or failed. The message never contains the key.
    Error { message: String },
    /// `abort` cancelled the request.
    Aborted,
}

/// The host could not start.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HostError {
    #[error("cannot build the HTTP client: {0}")]
    Client(String),
}

/// The egress proxy: policy, key, HTTP client, usage ledger and in-flight requests.
#[derive(Debug)]
pub struct EgressHost {
    policy: EgressPolicy,
    vault: CredentialVault,
    ledger: UsageLedger,
    client: reqwest::Client,
    in_flight: Mutex<InFlight>,
}

/// Cancel senders of the in-flight requests by request ID. Each registration has its own
/// token, so a finished request never removes a later request that reuses its ID.
/// `aborted_early` holds the last few IDs aborted before their request registered (the
/// sync `ai_abort` can overtake the async `ai_fetch`); such a request is never sent.
#[derive(Debug, Default)]
struct InFlight {
    next_token: u64,
    cancels: HashMap<String, (u64, oneshot::Sender<()>)>,
    aborted_early: VecDeque<String>,
}

impl EgressHost {
    pub fn new(
        policy: EgressPolicy,
        vault: CredentialVault,
        ledger: UsageLedger,
    ) -> Result<Self, HostError> {
        // No redirects: a redirect could lead outside the policy's single destination.
        let client = reqwest::Client::builder()
            .tls_backend_native()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(READ_TIMEOUT)
            .build()
            .map_err(|e| HostError::Client(e.to_string()))?;
        Ok(Self {
            policy,
            vault,
            ledger,
            client,
            in_flight: Mutex::default(),
        })
    }

    /// The vault, to store, check or delete the key. Reading it back is not possible.
    pub fn vault(&self) -> &CredentialVault {
        &self.vault
    }

    /// Forwards `request` and passes every [`ProxyEvent`] to `emit`. When `emit` returns
    /// `false` (the receiver is gone) the request is dropped. Returns when it is over.
    pub async fn fetch(&self, request: ProxyRequest, mut emit: impl FnMut(ProxyEvent) -> bool) {
        let (token, cancelled) = match self.register(&request.request_id) {
            Ok(Some(registered)) => registered,
            Ok(None) => {
                emit(ProxyEvent::Aborted);
                return;
            }
            Err(message) => {
                emit(ProxyEvent::Error { message });
                return;
            }
        };
        let _registration = Registration {
            host: self,
            id: request.request_id.clone(),
            token,
        };
        let mut usage = Usage::new();
        // Only an explicit `abort` cancels; the sender is never dropped while this runs.
        let finished = tokio::select! {
            Ok(()) = cancelled => false,
            () = self.forward(request, &mut emit, &mut usage) => true,
        };
        if !finished {
            emit(ProxyEvent::Aborted);
        }
        self.record(usage);
    }

    /// Cancels the in-flight request `request_id`. Returns whether there was one. If the
    /// request has not started yet, it is refused when it does.
    pub fn abort(&self, request_id: &str) -> bool {
        let mut in_flight = self.in_flight();
        if let Some((_, cancel)) = in_flight.cancels.remove(request_id) {
            return cancel.send(()).is_ok();
        }
        if in_flight.aborted_early.len() >= MAX_IN_FLIGHT {
            in_flight.aborted_early.pop_front();
        }
        in_flight.aborted_early.push_back(request_id.to_owned());
        false
    }

    /// Registers request `id` for `abort`. `Ok(None)` means it was aborted before it began.
    fn register(&self, id: &str) -> Result<Option<(u64, oneshot::Receiver<()>)>, String> {
        let valid = (1..=MAX_REQUEST_ID_BYTES).contains(&id.len())
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        if !valid {
            return Err("the request ID must be 1 to 64 letters, digits, '-' or '_'".into());
        }
        let mut in_flight = self.in_flight();
        if let Some(early) = in_flight.aborted_early.iter().position(|a| a == id) {
            in_flight.aborted_early.remove(early);
            return Ok(None);
        }
        if in_flight.cancels.contains_key(id) {
            return Err("a request with this ID is already in flight".into());
        }
        if in_flight.cancels.len() >= MAX_IN_FLIGHT {
            return Err(format!("at most {MAX_IN_FLIGHT} requests can be in flight"));
        }
        let token = in_flight.next_token;
        in_flight.next_token = token.wrapping_add(1);
        let (cancel, cancelled) = oneshot::channel();
        in_flight.cancels.insert(id.to_owned(), (token, cancel));
        Ok(Some((token, cancelled)))
    }

    /// The map is only ever inserted into or removed from, so a poisoned lock still
    /// holds consistent data.
    fn in_flight(&self) -> MutexGuard<'_, InFlight> {
        self.in_flight
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    async fn forward(
        &self,
        request: ProxyRequest,
        emit: &mut impl FnMut(ProxyEvent) -> bool,
        usage: &mut Usage,
    ) {
        let response = match self.send(request, usage).await {
            Ok(response) => response,
            Err(message) => {
                emit(ProxyEvent::Error { message });
                return;
            }
        };
        usage.record.status = Some(response.status().as_u16());
        // OpenRouter's generation ID; `request-id` is what Anthropic itself sends.
        let headers = response.headers();
        usage.record.request_id = ["x-generation-id", "request-id"]
            .iter()
            .find_map(|name| headers.get(*name)?.to_str().ok())
            .map(shorten);
        if emit(head(&response)) {
            stream_body(response, emit, usage).await;
        }
    }

    async fn send(&self, request: ProxyRequest, usage: &mut Usage) -> Result<Response, String> {
        let prepared = self.policy.prepare(request).map_err(|e| e.to_string())?;
        usage.record.model = Some(shorten(&prepared.model));
        let key = self.vault.key().map_err(|e| e.to_string())?;
        let mut bearer = HeaderValue::from_str(&format!("Bearer {key}"))
            .map_err(|_| "the stored API key is not a valid header value".to_owned())?;
        // Sensitive values are left out of `Debug` output and HTTP/2 header compression.
        bearer.set_sensitive(true);
        let mut headers = prepared.headers;
        headers.insert(AUTHORIZATION, bearer);
        let mut builder = self
            .client
            .request(prepared.method, prepared.url)
            .headers(headers);
        if let Some(body) = prepared.body {
            usage.record.request_bytes = byte_count(body.len());
            builder = builder.body(body);
        }
        usage.sent = true;
        builder
            .send()
            .await
            .map_err(|e| format!("the request failed: {e}"))
    }

    /// Writes the ledger line for a request that was sent. Ledger trouble never fails
    /// the request; it is reported on stderr (no headers, bodies or keys in it).
    /// ponytail: a small blocking file append on the async task (as is the keychain read
    /// in `send`); move both to `spawn_blocking` if the ledger grows past one line.
    fn record(&self, usage: Usage) {
        if !usage.sent {
            return;
        }
        let mut record = usage.record;
        record.duration_ms = u64::try_from(usage.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        if let Err(error) = self.ledger.append(&record) {
            eprintln!("tf-ai-host: cannot write the usage ledger: {error}");
        }
    }
}

/// Removes the request from the in-flight map when `fetch` ends, however it ends, unless
/// an abort already removed it and a new request took the same ID.
struct Registration<'a> {
    host: &'a EgressHost,
    id: String,
    token: u64,
}

impl Drop for Registration<'_> {
    fn drop(&mut self) {
        let mut in_flight = self.host.in_flight();
        if in_flight
            .cancels
            .get(&self.id)
            .is_some_and(|(token, _)| *token == self.token)
        {
            in_flight.cancels.remove(&self.id);
        }
    }
}

/// Usage of one request while it runs.
struct Usage {
    started: Instant,
    sent: bool,
    record: UsageRecord,
}

impl Usage {
    fn new() -> Self {
        Self {
            started: Instant::now(),
            sent: false,
            record: UsageRecord::default(),
        }
    }
}

fn head(response: &Response) -> ProxyEvent {
    let headers = response
        .headers()
        .iter()
        .filter(|(name, _)| *name != SET_COOKIE)
        .map(|(name, value)| {
            let value = String::from_utf8_lossy(value.as_bytes()).into_owned();
            (name.as_str().to_owned(), value)
        })
        .collect();
    ProxyEvent::Head {
        status: response.status().as_u16(),
        headers,
    }
}

async fn stream_body(
    mut response: Response,
    emit: &mut impl FnMut(ProxyEvent) -> bool,
    usage: &mut Usage,
) {
    let mut decoder = Utf8Chunks::default();
    loop {
        let event = match response.chunk().await {
            Ok(Some(bytes)) => {
                usage.record.response_bytes += byte_count(bytes.len());
                ProxyEvent::Chunk {
                    text: decoder.push(&bytes),
                }
            }
            Ok(None) => {
                let text = decoder.finish();
                if !text.is_empty() && !emit(ProxyEvent::Chunk { text }) {
                    return;
                }
                emit(ProxyEvent::End);
                return;
            }
            Err(e) => {
                emit(ProxyEvent::Error {
                    message: format!("the response failed: {e}"),
                });
                return;
            }
        };
        let empty = matches!(&event, ProxyEvent::Chunk { text } if text.is_empty());
        if !empty && !emit(event) {
            return;
        }
    }
}

fn byte_count(len: usize) -> u64 {
    u64::try_from(len).unwrap_or(u64::MAX)
}

/// Decodes a byte stream to text chunk by chunk, holding back an incomplete character at
/// the end of a chunk until the rest arrives.
#[derive(Debug, Default)]
struct Utf8Chunks {
    pending: Vec<u8>,
}

impl Utf8Chunks {
    fn push(&mut self, bytes: &[u8]) -> String {
        self.pending.extend_from_slice(bytes);
        // Skip past invalid sequences, so an incomplete character at the very end is held
        // back even when an invalid byte comes before it.
        let mut start = 0;
        let complete = loop {
            match std::str::from_utf8(&self.pending[start..]) {
                Ok(_) => break self.pending.len(),
                Err(e) => match e.error_len() {
                    None => break start + e.valid_up_to(),
                    Some(invalid) => start += e.valid_up_to() + invalid,
                },
            }
        };
        let rest = self.pending.split_off(complete);
        let done = std::mem::replace(&mut self.pending, rest);
        String::from_utf8_lossy(&done).into_owned()
    }

    fn finish(&mut self) -> String {
        String::from_utf8_lossy(&std::mem::take(&mut self.pending)).into_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use tokio::sync::mpsc;

    use super::*;
    use crate::ledger::tests::temp_ledger;
    use crate::test_server::{MockServer, Part, Reply};
    use crate::vault::tests::mock_vault;

    const KEY: &str = "test-key-not-real";

    fn host_for(server_origin: &str, ledger: &str) -> (EgressHost, PathBuf) {
        let vault = mock_vault();
        vault.set_key(KEY).unwrap();
        let (ledger, path) = temp_ledger(ledger);
        let host =
            EgressHost::new(EgressPolicy::for_test_origin(server_origin), vault, ledger).unwrap();
        (host, path)
    }

    fn request(id: &str, url: &str) -> ProxyRequest {
        ProxyRequest {
            request_id: id.into(),
            method: "POST".into(),
            url: url.into(),
            headers: vec![
                ("anthropic-version".into(), "2023-06-01".into()),
                ("content-type".into(), "application/json".into()),
                ("x-api-key".into(), "test-key-from-js".into()),
                ("authorization".into(), "Bearer test-token".into()),
                ("cookie".into(), "session=1".into()),
            ],
            body: Some(r#"{"model":"anthropic/claude-opus-5","max_tokens":16}"#.into()),
        }
    }

    /// Runs `fetch` to completion and returns every event.
    async fn fetch_all(host: &EgressHost, request: ProxyRequest) -> Vec<ProxyEvent> {
        let mut events = Vec::new();
        host.fetch(request, |event| {
            events.push(event);
            true
        })
        .await;
        events
    }

    /// Starts `fetch` on a task; events arrive on the returned receiver.
    fn spawn_fetch(
        host: &Arc<EgressHost>,
        request: ProxyRequest,
    ) -> (
        mpsc::UnboundedReceiver<ProxyEvent>,
        tokio::task::JoinHandle<()>,
    ) {
        let (sender, receiver) = mpsc::unbounded_channel();
        let host = Arc::clone(host);
        let task = tokio::spawn(async move {
            host.fetch(request, |event| sender.send(event).is_ok())
                .await;
        });
        (receiver, task)
    }

    fn header<'a>(event: &'a ProxyEvent, name: &str) -> Option<&'a str> {
        let ProxyEvent::Head { headers, .. } = event else {
            return None;
        };
        headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    fn chunk(text: &str) -> ProxyEvent {
        ProxyEvent::Chunk { text: text.into() }
    }

    fn ledger_lines(path: &PathBuf) -> Vec<serde_json::Value> {
        std::fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[tokio::test]
    async fn injects_the_stored_key_and_strips_caller_credentials() {
        let mut reply = Reply::ok(b"data: hi\n\n");
        reply.headers.push(("x-generation-id", "gen-test-1"));
        let server = MockServer::start(reply).await;
        let (host, ledger) = host_for(&server.origin, "inject");

        let events = fetch_all(
            &host,
            request("r1", &server.url("/api/v1/messages?beta=true")),
        )
        .await;

        let got = server.received().unwrap();
        assert_eq!(got.method, "POST");
        assert_eq!(got.target, "/api/v1/messages?beta=true");
        assert_eq!(got.header("authorization"), [format!("Bearer {KEY}")]);
        assert!(got.header("x-api-key").is_empty());
        assert!(got.header("cookie").is_empty());
        assert_eq!(got.header("anthropic-version"), ["2023-06-01"]);
        assert_eq!(got.header("content-type"), ["application/json"]);
        assert_eq!(
            got.body,
            r#"{"model":"anthropic/claude-opus-5","max_tokens":16}"#
        );

        assert_eq!(events.len(), 3, "{events:?}");
        assert!(matches!(events[0], ProxyEvent::Head { status: 200, .. }));
        assert_eq!(
            header(&events[0], "content-type"),
            Some("text/event-stream")
        );
        assert_eq!(events[1..], [chunk("data: hi\n\n"), ProxyEvent::End]);

        let lines = ledger_lines(&ledger);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["requestId"], "gen-test-1");
        assert_eq!(lines[0]["model"], "anthropic/claude-opus-5");
        assert_eq!(lines[0]["status"], 200);
        assert_eq!(lines[0]["requestBytes"], 51);
        assert_eq!(lines[0]["responseBytes"], 10);
        assert!(!std::fs::read_to_string(&ledger).unwrap().contains(KEY));
    }

    #[tokio::test]
    async fn streams_chunks_before_the_body_ends() {
        let (release, wait) = oneshot::channel();
        let server = MockServer::start(Reply {
            status: 200,
            headers: vec![("content-type", "text/event-stream")],
            parts: vec![
                Part::Bytes(b"event: one\n\n"),
                Part::Wait(wait),
                Part::Bytes(b"event: two\n\n"),
            ],
        })
        .await;
        let host = Arc::new(host_for(&server.origin, "stream").0);

        let (mut events, task) = spawn_fetch(&host, request("r1", &server.url("/api/v1/messages")));

        assert!(matches!(
            events.recv().await,
            Some(ProxyEvent::Head { status: 200, .. })
        ));
        // The server is still holding the rest of the body back.
        assert_eq!(events.recv().await, Some(chunk("event: one\n\n")));
        release.send(()).unwrap();
        assert_eq!(events.recv().await, Some(chunk("event: two\n\n")));
        assert_eq!(events.recv().await, Some(ProxyEvent::End));
        task.await.unwrap();
        assert_eq!(events.recv().await, None);
    }

    #[tokio::test]
    async fn abort_cancels_an_in_flight_request() {
        let (_hold, wait) = oneshot::channel::<()>();
        let server = MockServer::start(Reply {
            status: 200,
            headers: vec![],
            parts: vec![Part::Bytes(b"partial"), Part::Wait(wait)],
        })
        .await;
        let (host, ledger) = host_for(&server.origin, "abort");
        let host = Arc::new(host);

        let (mut events, task) =
            spawn_fetch(&host, request("r-abort", &server.url("/api/v1/messages")));
        assert!(matches!(events.recv().await, Some(ProxyEvent::Head { .. })));
        assert_eq!(events.recv().await, Some(chunk("partial")));

        assert!(host.abort("r-abort"));
        assert_eq!(events.recv().await, Some(ProxyEvent::Aborted));
        task.await.unwrap();
        assert_eq!(events.recv().await, None);
        assert!(!host.abort("r-abort"), "the request is no longer in flight");
        assert_eq!(ledger_lines(&ledger)[0]["responseBytes"], 7);
    }

    #[tokio::test]
    async fn abort_of_an_unknown_request_does_nothing() {
        let (host, _) = host_for("http://127.0.0.1:9", "abort-unknown");
        assert!(!host.abort("nope"));
    }

    #[tokio::test]
    async fn passes_non_2xx_responses_through() {
        let server = MockServer::start(Reply {
            status: 429,
            headers: vec![
                ("retry-after", "7"),
                ("content-type", "application/json"),
                ("request-id", "req_1"),
            ],
            parts: vec![Part::Bytes(br#"{"type":"error"}"#)],
        })
        .await;
        let (host, ledger) = host_for(&server.origin, "non2xx");

        let events = fetch_all(&host, request("r1", &server.url("/api/v1/messages"))).await;

        assert!(matches!(events[0], ProxyEvent::Head { status: 429, .. }));
        assert_eq!(header(&events[0], "retry-after"), Some("7"));
        assert_eq!(events[1..], [chunk(r#"{"type":"error"}"#), ProxyEvent::End]);
        let line = &ledger_lines(&ledger)[0];
        assert_eq!(line["status"], 429);
        // Without OpenRouter's generation ID, the `request-id` header is recorded.
        assert_eq!(line["requestId"], "req_1");
    }

    #[tokio::test]
    async fn does_not_follow_redirects_or_pass_cookies_back() {
        let server = MockServer::start(Reply {
            status: 302,
            headers: vec![("location", "https://evil.example/"), ("set-cookie", "a=1")],
            parts: vec![],
        })
        .await;
        let (host, _) = host_for(&server.origin, "redirect");

        let events = fetch_all(&host, request("r1", &server.url("/api/v1/messages"))).await;

        assert!(matches!(events[0], ProxyEvent::Head { status: 302, .. }));
        assert_eq!(
            header(&events[0], "location"),
            Some("https://evil.example/")
        );
        assert_eq!(header(&events[0], "set-cookie"), None);
        assert_eq!(events[1..], [ProxyEvent::End]);
    }

    #[tokio::test]
    async fn refuses_a_disallowed_destination_without_connecting() {
        let server = MockServer::start(Reply::ok(b"never sent")).await;
        let (host, ledger) = host_for(&server.origin, "refuse");

        let events = fetch_all(
            &host,
            request("r1", "https://openrouter.ai/api/v1/messages"),
        )
        .await;

        assert!(
            matches!(&events[..], [ProxyEvent::Error { message }] if message.contains("not allowed"))
        );
        assert!(server.received().is_none());
        assert!(ledger_lines(&ledger).is_empty());
    }

    #[tokio::test]
    async fn reports_a_missing_key_without_connecting() {
        let server = MockServer::start(Reply::ok(b"never sent")).await;
        let (host, _) = host_for(&server.origin, "nokey");
        host.vault().delete_key().unwrap();

        let events = fetch_all(&host, request("r1", &server.url("/api/v1/messages"))).await;

        assert_eq!(
            events,
            [ProxyEvent::Error {
                message: "no API key is stored".into()
            }]
        );
        assert!(server.received().is_none());
    }

    #[tokio::test]
    async fn rejects_bad_and_duplicate_request_ids() {
        let (_hold, wait) = oneshot::channel::<()>();
        let server = MockServer::start(Reply {
            status: 200,
            headers: vec![],
            parts: vec![Part::Wait(wait)],
        })
        .await;
        let host = Arc::new(host_for(&server.origin, "ids").0);
        let url = server.url("/api/v1/messages");

        let long = "x".repeat(65);
        for id in ["", "has space", "slash/", long.as_str()] {
            let events = fetch_all(&host, request(id, &url)).await;
            assert!(
                matches!(&events[..], [ProxyEvent::Error { message }] if message.contains("request ID")),
                "{id:?}"
            );
        }

        let (mut first, task) = spawn_fetch(&host, request("same", &url));
        assert!(matches!(first.recv().await, Some(ProxyEvent::Head { .. })));
        let events = fetch_all(&host, request("same", &url)).await;
        assert!(
            matches!(&events[..], [ProxyEvent::Error { message }] if message.contains("already in flight"))
        );
        assert!(host.abort("same"));
        task.await.unwrap();
    }

    #[tokio::test]
    async fn stops_when_the_receiver_is_gone() {
        let server = MockServer::start(Reply::ok(b"data: hi\n\n")).await;
        let (host, ledger) = host_for(&server.origin, "gone");

        let mut events = Vec::new();
        host.fetch(request("r1", &server.url("/api/v1/messages")), |event| {
            events.push(event);
            false
        })
        .await;

        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], ProxyEvent::Head { .. }));
        assert_eq!(ledger_lines(&ledger).len(), 1);
    }

    #[tokio::test]
    async fn reports_a_connection_failure() {
        // Bind and drop a listener to find a port that refuses connections.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let origin = format!("http://127.0.0.1:{port}");
        let (host, ledger) = host_for(&origin, "refused");

        let events = fetch_all(&host, request("r1", &format!("{origin}/api/v1/messages"))).await;

        assert!(
            matches!(&events[..], [ProxyEvent::Error { message }] if message.starts_with("the request failed"))
        );
        let lines = ledger_lines(&ledger);
        assert_eq!(lines[0]["status"], serde_json::Value::Null);
    }

    #[test]
    fn events_serialize_to_the_ipc_shape() {
        let head = ProxyEvent::Head {
            status: 200,
            headers: vec![("a".into(), "b".into())],
        };
        assert_eq!(
            serde_json::to_string(&head).unwrap(),
            r#"{"kind":"head","status":200,"headers":[["a","b"]]}"#
        );
        assert_eq!(
            serde_json::to_string(&chunk("x")).unwrap(),
            r#"{"kind":"chunk","text":"x"}"#
        );
        assert_eq!(
            serde_json::to_string(&ProxyEvent::End).unwrap(),
            r#"{"kind":"end"}"#
        );
        assert_eq!(
            serde_json::to_string(&ProxyEvent::Aborted).unwrap(),
            r#"{"kind":"aborted"}"#
        );
    }

    #[test]
    fn utf8_chunks_hold_back_a_split_character() {
        let mut decoder = Utf8Chunks::default();
        let euro = "€".as_bytes();
        assert_eq!(decoder.push(&[b'a', euro[0]]), "a");
        assert_eq!(decoder.push(&euro[1..2]), "");
        assert_eq!(decoder.push(&[euro[2], b'b']), "€b");
        assert_eq!(decoder.push(&[0xff, b'c']), "\u{fffd}c");
        assert_eq!(decoder.push(&[0xff, b'd', euro[0]]), "\u{fffd}d");
        assert_eq!(decoder.push(&euro[1..]), "€");
        assert_eq!(decoder.push(&euro[..1]), "");
        assert_eq!(decoder.finish(), "\u{fffd}");
        assert_eq!(decoder.finish(), "");
    }

    #[test]
    fn a_ledger_failure_does_not_fail_the_request() {
        let vault = mock_vault();
        let (ledger, path) = temp_ledger("ledger-fails");
        std::fs::create_dir_all(path.parent().unwrap().parent().unwrap()).unwrap();
        std::fs::write(path.parent().unwrap(), b"").unwrap();
        let host = EgressHost::new(EgressPolicy::openrouter(), vault, ledger).unwrap();
        let mut usage = Usage::new();
        usage.sent = true;
        host.record(usage);
    }

    #[test]
    fn limits_the_requests_in_flight() {
        let (host, _) = host_for("http://127.0.0.1:9", "in-flight-cap");
        let held: Vec<_> = (0..MAX_IN_FLIGHT)
            .map(|i| host.register(&format!("r{i}")).unwrap().unwrap())
            .collect();
        let refused = host.register("one-more").unwrap_err();
        assert!(refused.contains("at most 16"), "{refused}");
        assert!(host.abort("r0"));
        assert!(host.register("one-more").is_ok());
        drop(held);
    }

    #[tokio::test]
    async fn an_abort_that_overtakes_its_request_stops_it_before_sending() {
        let server = MockServer::start(Reply::ok(b"never sent")).await;
        let (host, ledger) = host_for(&server.origin, "abort-early");

        assert!(!host.abort("early"));
        let events = fetch_all(&host, request("early", &server.url("/api/v1/messages"))).await;

        assert_eq!(events, [ProxyEvent::Aborted]);
        assert!(server.received().is_none());
        assert!(ledger_lines(&ledger).is_empty());
        // The early abort is used up: the ID works again.
        let (_token, _cancelled) = host.register("early").unwrap().unwrap();
    }

    #[test]
    fn remembers_only_the_latest_early_aborts() {
        let (host, _) = host_for("http://127.0.0.1:9", "abort-early-cap");
        for i in 0..=MAX_IN_FLIGHT {
            assert!(!host.abort(&format!("e{i}")));
        }
        assert!(
            host.register("e0").unwrap().is_some(),
            "the oldest was forgotten"
        );
        assert!(
            host.register(&format!("e{MAX_IN_FLIGHT}"))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn a_finished_request_never_unregisters_a_newer_one_with_its_id() {
        let (host, _) = host_for("http://127.0.0.1:9", "reuse-id");
        let (old_token, _old) = host.register("same").unwrap().unwrap();
        assert!(host.abort("same"));
        let (_new_token, mut new) = host.register("same").unwrap().unwrap();

        drop(Registration {
            host: &host,
            id: "same".into(),
            token: old_token,
        });

        assert!(new.try_recv().is_err(), "the new request was not cancelled");
        assert!(host.abort("same"), "the new request is still registered");
    }
}
