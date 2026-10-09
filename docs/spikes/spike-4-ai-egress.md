# Spike 4: AI egress proxy and webview client

Tests [ADR-0009](../adr/0009-ai-orchestration-typescript-sdk-rust-egress.md) (with its 2026-10-05 OpenRouter amendment) and the routes of [ADR-0011](../adr/0011-model-configuration.md). Plan: [M0 Steps 9.1 and 9.2](../../plans/typefaced-m0-foundations-and-spikes.md). Code: [`crates/tf-ai-host`](../../crates/tf-ai-host/), [`apps/desktop/src-tauri/src/ai.rs`](../../apps/desktop/src-tauri/src/ai.rs), [`packages/ai`](../../packages/ai/), [`apps/desktop/src/ai-spike/`](../../apps/desktop/src/ai-spike/).

**Verdict in one line.** Pass: ADR-0009 is Accepted. Everything ADR-0009 needs works: the official TypeScript SDK runs in the webview on a custom `fetch` that goes through the Rust proxy to OpenRouter's Anthropic-compatible Messages API, the response streams in pieces (OpenRouter's keep-alive comments and `[DONE]` included), tool input is validated before any tool runs, the mutation tool waits for approval, and the proxy forwards only the one endpoint, headers and body fields the client sends. The live smoke test with the user's OpenRouter key passed (streamed answer, approved rename), and the debug build's CSP blocks direct network access from the page.

## Question

Can the in-app agent run on the official `@anthropic-ai/sdk` in the webview while the API key stays on the Rust side?

- Does the SDK accept a custom `fetch` that forwards over Tauri IPC, and does streaming survive the hop (the `fetch` must resolve on the response head)?
- Does the key stay out of JavaScript and out of the built bundle?
- Are non-allowlisted destinations, headers and body fields refused?
- Does invalid tool input never run, and does the mutation tool need approval?

**Provider change (user decision 2026-10-05).** The project uses an **OpenRouter** API key, not a direct Anthropic key. The SDK stays and points at OpenRouter's Anthropic-compatible endpoint; the model is picked per task by the app, with OpenRouter fallbacks.

## OpenRouter facts, verified from OpenRouter's own reference

Read on 2026-10-05 from OpenRouter's OpenAPI document and documentation pages (not from summaries):

- OpenAPI: <https://openrouter.ai/openapi.json> (path `/messages`, schemas `MessagesFallbackParam`, `ProviderPreferences`, `MessagesOutputConfig`).
- Endpoint page: <https://openrouter.ai/docs/api/api-reference/anthropic-messages/create-a-message.md>
- Streaming: <https://openrouter.ai/docs/api/reference/streaming.md>
- Claude Code / Agent SDK integration: <https://openrouter.ai/docs/cookbook/coding-agents/claude-code-integration.md>, <https://openrouter.ai/docs/guides/community/anthropic-agent-sdk.md>
- Privacy: <https://openrouter.ai/docs/guides/privacy/data-collection.md>, <https://openrouter.ai/docs/guides/privacy/provider-logging.md>, <https://openrouter.ai/docs/guides/features/zdr.md>
- Models: `GET https://openrouter.ai/api/v1/models` (public, no key).

| # | Question | Finding | What the code does |
|---|---|---|---|
| 1 | `fallbacks` on `/api/v1/messages` | An array (or null) of objects; "Each entry accepts only `model`. Maximum of 3 entries." It is handled by OpenRouter's multi-model routing, not Anthropic's server-side fallbacks, is tried "if the primary model fails or refuses", and cannot be combined with `models`. The pinned SDK types `fallbacks` as `Array<BetaFallbackParam> \| 'default'`, and `BetaFallbackParam.model` takes a string, so `[{ model }]` needs no cast. | Sends `fallbacks: [{ model }, …]` from the route table; Rust allows at most 3, each with only `model` (`deny_unknown_fields`), and refuses `"default"`. |
| 2 | `anthropic-version` / `anthropic-beta` | Neither header appears in the OpenAPI document. OpenRouter documents Claude Code and the Anthropic Agent SDK talking to `https://openrouter.ai/api` with their native protocol (both always send `anthropic-version`), and says the one beta it needs (fast mode) "is injected automatically". | `anthropic-version` passes through; the client sends no beta and Rust refuses any `anthropic-beta` header. |
| 3 | `thinking`, `output_config.effort`, `eager_input_streaming` | `thinking` accepts `{type: "adaptive"}` (also `enabled`, `disabled`, `between_tools`); `output_config.effort` accepts `low`…`max`. The custom-tool schema lists `name`, `description`, `input_schema`, `cache_control`, `defer_loading`, `strict`, `type` — no `eager_input_streaming`. | Adaptive thinking and effort per route are sent; `eager_input_streaming` is dropped (tool input is still validated with Ajv). |
| 4 | `count_tokens`, `GET /models` | The OpenAPI document has no `count_tokens` path. `GET /models` exists but the client never calls it. Model IDs contain `/`. | Rust allows only `POST /api/v1/messages` (query `beta=true`, which the SDK's beta call adds). |
| 5 | Model IDs | From the public model list (see the routes below). All chosen IDs list `tools` and `reasoning_effort` as supported parameters. | Rust accepts only `anthropic/<model>` IDs in lower case without a `:variant` suffix (`:online` is the web-search plugin; `:nitro`, `:floor`, `:free` pick providers), so other vendors and OpenRouter's routers (`openrouter/*`) are refused. |
| 6 | Stream format | "OpenRouter occasionally sends comments to prevent connection timeouts", e.g. `: OPENROUTER PROCESSING`; the Messages endpoint's SSE sentinel is `[DONE]` (`x-speakeasy-sse-sentinel` in the endpoint page). The Messages API reports usage in `message_delta`. The generation ID is in the `X-Generation-Id` header. | Every `streamed()` turn fixture now starts with the keep-alive comment and ends with `data: [DONE]`; one test also splits a comment across network chunks. The pinned SDK's parser skips `:` lines and ignores the event-less `[DONE]`. The ledger records `x-generation-id`. |
| 7 | Data handling | `provider.data_collection`: `allow` (default: "allow providers which store user data non-transiently and may train on it") or `deny` ("use only providers which do not collect user data"); `provider.zdr`: route only to zero-data-retention endpoints. The same controls exist account-wide in the privacy settings; OpenRouter itself stores prompts only if the user opts in to logging. | Not sent (the user decides). Rust refuses the `provider` field for now; the account settings are the control (ADR-0009 accepted risk 3). |

## Model routes (ADR-0011)

| Task | Model | Fallbacks | Effort |
|---|---|---|---|
| `chat` | `anthropic/claude-sonnet-5.5` | `anthropic/claude-sonnet-5`, `anthropic/claude-sonnet-4.6` | `low` |
| `agent` (the spike loop) | `anthropic/claude-opus-5.5` | `anthropic/claude-opus-5`, `anthropic/claude-sonnet-5.5` | `medium` |

## Setup

| Item | Value |
|---|---|
| Machine | HP Pavilion Gaming Laptop 17-cd0xxx, Intel Core i7-9750H (6 cores / 12 threads), 32 GB RAM (the Spike 3 machine) |
| OS and WebView | Windows 11 Pro build 26300; WebView2 |
| Toolchain | rustc 1.98.1, Node 24.14.0, pnpm 9.15.9 |
| Rust crates (`tf-ai-host`) | reqwest 0.13.5 (`native-tls`, `http2`, `system-proxy`; SChannel and the OS certificate store), keyring-core 1.0.0, windows-native-keyring-store 1.1.0, tokio 1.53.1, serde_json 1 |
| npm packages (`@typefaced/ai`) | `@anthropic-ai/sdk` 0.131.0 (MIT), `ajv` 8.20.0 (MIT), `@tauri-apps/api` 2.12.0; transitive `fast-sha256` 1.3.0 (Unlicense, allowed by the user on 2026-10-05) |
| Provider and request settings | OpenRouter, `baseURL` `https://openrouter.ai/api`, the routes above, adaptive thinking, streaming, `max_tokens` 64,000, at most 8 tool-runner iterations |

## Method

**Rust half (Step 9.1, changed in 9.2 for OpenRouter).** `tf-ai-host` is tested without network access against a scripted local HTTP server (`test_server.rs`), which can pause mid-body:

- `CredentialVault`: keyring-core's mock store, and on Windows a round trip through Credential Manager under service `com.typefaced.desktop.test` with a random account that is deleted afterwards. The production entry (account `openrouter-api-key`) is never touched.
- `EgressPolicy`: only `POST https://openrouter.ai/api/v1/messages`. The bypass tests cover `openrouter.ai.evil.com`, `openrouter.ai@evil.com`, `evil.com@openrouter.ai`, user info, other ports, `http://`, a trailing dot, sub-domains, `api.anthropic.com` (now refused), path tricks (`..`, `%2e%2e`, backslash, `%2F`, trailing slash, case), other OpenRouter endpoints (`chat/completions`, `responses`, `count_tokens`, `models`, `keys`, presets), other methods, and every query except `beta=true`.
- Headers: only `anthropic-version`, `content-type` and `accept` pass; `x-api-key`, `authorization`, `cookie`, `HTTP-Referer` and `X-Title` from the webview are dropped; any `anthropic-beta` is refused.
- Body: one JSON object without duplicate keys; only `model`, `max_tokens`, `messages`, `system`, `tools`, `thinking`, `output_config`, `stream`, `fallbacks` at the top level (names compared with JSON escapes decoded), so OpenRouter's `plugins`, `provider`, `models`, `route`, `session_id`, `trace`, `safeguards`, `metadata`, `user`, `stop_server_tools_when`, `service_tier` and `speed` are refused; a required `anthropic/<model>` ID without a `:variant`; at most 3 fallbacks with only `model` and the same ID check; `max_tokens` at most 64000; `thinking` only adaptive; no server tools (Anthropic's or `openrouter:*`), no `mcp_servers`, no content the API would fetch from a URL or file.
- `EgressHost`: `Authorization: Bearer <key>` injection, streaming passthrough (chunks before the body ends), abort, non-2xx passthrough, no redirects, no `set-cookie`, the ledger line (`x-generation-id`, else `request-id`; the requested model).

**Webview half (Step 9.2).** `@typefaced/ai` is tested under Vitest with the Tauri IPC mocked by a fake proxy that replays scripted channel events, including pause points:

- `tauriFetch`: resolves on the head while the body is still open; keeps the body open until the channel's `end`, even when the `ai_fetch` command resolves first (Tauri delivers large channel messages in a later IPC round trip); errors before and after the head; abort before and after the head reaches `ai_abort`; cancelling the body aborts too; events out of protocol order are refused.
- `runAgent`: the SDK's streamed beta tool runner over `tauriFetch`, with SSE fixtures in the Messages API format wrapped as OpenRouter sends them (keep-alive comment, `[DONE]`). Paths: the request settings and URL; the `chat` route; a stream with comments inside and between chunks; a tool call; tool input that fails validation; approval declined; approval given; a `refusal`; a tool call in a turn that did not stop for `tool_use` (`max_tokens`, context window); the iteration limit; abort. Both requests of the tool-call test (the second carries the tool result) are checked against the Rust rules: body fields, model-ID form, at most 3 fallbacks with only `model`, no `anthropic-beta`.
- The dev-only page `#/ai-spike` (only when `import.meta.env.DEV`, like `#/bench`) is tested with the bindings and `runAgent` mocked: key stored and deleted; approval given and declined; Stop and leaving the page decline a pending approval and abort the request; a second approval in the same turn is declined; Run is disabled while running; a proxy refusal shows its reason.

**Key handling.** The SDK gets `apiKey: "injected-by-rust"` and `dangerouslyAllowBrowser: true`; Rust drops every incoming `x-api-key` and `authorization` and adds the stored key as `Authorization: Bearer`. The key field ("OpenRouter API key") is a password input that is cleared before `ai_set_key` is called; the page only ever shows "Key stored: yes/no".

## Results

### Automated (no network)

| Suite | Result |
|---|---|
| `cargo test -p tf-ai-host` | 52 passed |
| `cargo xtask coverage` | `tf-ai-host` lines 97.97% (`policy.rs` 100%, `body.rs` 97.6%, `host.rs` 98.0%, `ledger.rs` 98.3%, `vault.rs` 97.1%); adapter gate ≥ 80% |
| `pnpm --filter @typefaced/ai test:ci` | 33 passed; 97.8% statements, 95% branches |
| `pnpm --filter @typefaced/desktop test:ci` | 52 passed; 98.3% statements |
| Bundle check: `! grep -rE 'sk-ant-[A-Za-z0-9_-]{16,}\|sk-or-' apps/desktop/dist` | no match (on the debug build's `dist`) |

What the tests show against the ADR-0009 pass criteria:

- **Streaming in pieces through the proxy:** Rust emits each network read as a chunk before the body ends; `tauriFetch` resolves on the head and hands each chunk to the SDK as it arrives; OpenRouter's comments and `[DONE]` do not break the SDK's parser.
- **Key only in Credential Manager:** no command returns the key; the webview sends only the placeholder; the field is cleared before the IPC call; the built bundle has no key-shaped string.
- **Non-allowlisted destinations rejected:** only OpenRouter's Messages endpoint passes; headers, betas, body fields and model IDs are checked too.
- **Invalid tool input never runs:** the approval callback is never called and the font is unchanged; the model receives an `INVALID_JSON` error result.
- **The mutation tool needs approval:** declined leaves the font unchanged and returns "The user declined the change"; approved changes it.
- **Security review without open CRITICAL or HIGH findings:** four reviews on opus (security, Rust, TypeScript, general) over the whole diff after the OpenRouter rework. No CRITICAL. The two HIGH findings were fixed: model `:variant` suffixes passed the model check (`:online` turns on OpenRouter's web plugin; now only `anthropic/<model>` without `:`), and stale Anthropic text in `CONTEXT.md` and the crate description. The MEDIUM fixes: `max_tokens` capped at 64 000 and only adaptive `thinking`, the last allowed turn stops before its tools run, `authToken: null`, a dedicated `FetchedSource` error. Open MEDIUM/LOW items are in the follow-ups.

### Live smoke test (GATE): pass

Run by the user on 2026-10-08 under `tauri dev` on the `#/ai-spike` page, key stored through the page (Windows Credential Manager, account `openrouter-api-key`).

- **Billing refusal first.** The first two runs got HTTP 402 from OpenRouter: it reserves credit for the full `max_tokens` (64 000) before running a request, and the account's balance covered fewer. The error reached the page intact through the proxy. The user added credits; no code change.
- **Plain prompt:** a streamed answer from the agent route (status 200, 7.7 s).
- **Prompt "Rename the font family to Testface":** the model called `set_family_name`, the approval dialog appeared, the user approved, and the family name changed to Testface. Two requests, as expected for one tool round trip.

Ledger lines (`%APPDATA%com.typefaced.desktopai-usage.jsonl`; no body, prompt or key in it):

| Generation ID | Model (requested) | Status | Duration | Request / response bytes |
|---|---|---|---|---|
| gen-1791492084-voIdin1poG0WSTs9Syl8 | anthropic/claude-opus-5.5 | 200 | 7686 ms | 1172 / 4034 |
| gen-1791492347-dvX1clVZfbor1LBqq4Ey | anthropic/claude-opus-5.5 | 200 | 7902 ms | 1164 / 1911 |
| gen-1791492363-jUtZCCi2yKmSebacUJqP | anthropic/claude-opus-5.5 | 200 | 2631 ms | 1485 / 1810 |

The model that served each request and the cost were not recorded (OpenRouter's activity page has both).

### Debug-build CSP check: pass

Run by the user on 2026-10-09 in the devtools console of the debug build (`tauri build --debug --no-bundle`, bundled frontend): `fetch('https://openrouter.ai/api/v1/models')` was blocked ("violates the following Content Security Policy directive: \"connect-src ipc: http://ipc.localhost\"") and the promise rejected with `TypeError: Failed to fetch`. The same call succeeds under `tauri dev`, where the page comes from the Vite server and the CSP is not applied; that is expected, and the AI page exists only there.

## Decision

Both manual checks passed: ADR-0009 is Accepted (2026-10-09) with the three accepted risks it records (key replace or delete from the webview, closed by the M3 native credential prompt; the system proxy is honoured, for corporate networks; prompts go to the upstream provider OpenRouter selects, controlled by the account's privacy settings).

## Follow-ups

- **Ajv under the CSP.** Ajv compiles validators with `new Function`, which the shipped CSP blocks. The spike page is dev-only and runs without the CSP; production code (M3) must use Ajv standalone (precompiled) validators.
- **Data-handling preferences.** Whether to send `provider: {data_collection: "deny"}` or `zdr: true` per request (Rust would then allow exactly that `provider` shape) is the user's decision; until then the account settings apply.
- **Fallback turns.** Which model served a turn is visible only in the response (`message.model`); the ledger records the requested model. Record the served model when the ledger moves past the stub.
- **Unparseable tool JSON.** JSON the SDK cannot parse at all rejects the turn; the spike reports it as a failure. Production code should re-issue the turn (capped), as the `claude-api` skill describes.
- **Allowlists follow the client.** A new request feature (a beta, `tool_choice`, an OpenRouter extension, …) needs a change in `tf-ai-host` with a test, on purpose.
- **Policy refusals look like connection errors to the SDK.** A refusal before the head rejects `fetch`, so the SDK retries it twice and reports "Connection error." with the reason as its `cause` (the spike page shows it). Production code could send refusals as an HTTP-like 403 head instead.
- **Dev page loader.** `main.tsx` imports the dev pages without a `.catch`, as Step 8 did; fine for dev-only pages.
- **New commands for Step 12 to fold into `AGENTS.md`:** `pnpm --filter @typefaced/ai test`; the spike page at `#/ai-spike` under `pnpm --filter @typefaced/desktop tauri dev`.
