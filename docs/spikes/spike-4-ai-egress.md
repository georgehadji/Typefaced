# Spike 4: AI egress proxy and webview client

Tests [ADR-0009](../adr/0009-ai-orchestration-typescript-sdk-rust-egress.md). Plan: [M0 Steps 9.1 and 9.2](../../plans/typefaced-m0-foundations-and-spikes.md). Code: [`crates/tf-ai-host`](../../crates/tf-ai-host/), [`apps/desktop/src-tauri/src/ai.rs`](../../apps/desktop/src-tauri/src/ai.rs), [`packages/ai`](../../packages/ai/), [`apps/desktop/src/ai-spike/`](../../apps/desktop/src/ai-spike/).

**Verdict in one line.** Pending the two manual checks. Without network access everything ADR-0009 needs works: the official TypeScript SDK runs in the webview on a custom `fetch` that goes through the Rust proxy, the response streams in pieces, tool input is validated before any tool runs, the mutation tool waits for approval, and the proxy forwards only the endpoints, beta values and body fields the client sends. The live smoke test (GATE) and the debug-build CSP check are **pending (manual)**.

## Question

Can the in-app agent run on the official `@anthropic-ai/sdk` in the webview while the Claude API key stays on the Rust side?

- Does the SDK accept a custom `fetch` that forwards over Tauri IPC, and does streaming survive the hop (the `fetch` must resolve on the response head)?
- Does the key stay out of JavaScript and out of the built bundle?
- Are non-allowlisted destinations, betas and body fields refused?
- Does invalid tool input never run, and does the mutation tool need approval?

## Setup

| Item | Value |
|---|---|
| Machine | HP Pavilion Gaming Laptop 17-cd0xxx, Intel Core i7-9750H (6 cores / 12 threads), 32 GB RAM (the Spike 3 machine) |
| OS and WebView | Windows 11 Pro build 26300; WebView2 |
| Toolchain | rustc 1.98.1, Node 24.14.0, pnpm 9.15.9 |
| Rust crates (`tf-ai-host`) | reqwest 0.13.5 (`native-tls`, `http2`, `system-proxy`; SChannel and the OS certificate store), keyring-core 1.0.0, windows-native-keyring-store 1.1.0, tokio 1.53.1, serde_json 1 |
| npm packages (`@typefaced/ai`) | `@anthropic-ai/sdk` 0.131.0 (MIT), `ajv` 8.20.0 (MIT), `@tauri-apps/api` 2.12.0 |
| Model and request settings | `claude-opus-5-5` (the `claude-api` skill's current default; the plan said `claude-opus-5`), adaptive thinking, effort `medium` (set explicitly; the model's default), streaming, `eager_input_streaming: true` on both client tools, `fallbacks: "default"` with beta `server-side-fallback-2026-07-01`, `max_tokens` 64,000, at most 8 tool-runner iterations |

## Method

**Rust half (Step 9.1, extended in 9.2).** `tf-ai-host` is tested without network access against a scripted local HTTP server (`test_server.rs`), which can pause mid-body:

- `CredentialVault`: keyring-core's mock store, and on Windows a round trip through Credential Manager under service `com.typefaced.desktop.test` with a random account that is deleted afterwards. The production entry is never touched.
- `EgressPolicy`: origin, endpoints (exact match after URL normalisation), methods, query parameters, header allowlist, credential stripping, body size, body check (duplicate keys, server tools, `mcp_servers`, fetched content sources).
- Step 9.2 added two allowlists (tests first): `anthropic-beta` may hold only `server-side-fallback-2026-07-01` (every comma-separated value is checked; a header line with any other value is refused), and a POST body may hold only the top-level fields `model`, `max_tokens`, `messages`, `system`, `tools`, `thinking`, `output_config`, `stream` and `fallbacks`, and `fallbacks` only as the string `"default"` (a list could carry per-model overrides the check does not look into). Field names are compared with JSON escapes decoded; a duplicate field is refused.
- `EgressHost`: key injection, streaming passthrough (chunks before the body ends), abort, non-2xx passthrough, no redirects, no `set-cookie`, the ledger line.

**Webview half (Step 9.2).** `@typefaced/ai` is tested under Vitest with the Tauri IPC mocked by a fake proxy that replays scripted channel events, including pause points:

- `tauriFetch`: resolves on the head while the body is still open; keeps the body open until the channel's `end`, even when the `ai_fetch` command resolves first (Tauri delivers large channel messages in a later IPC round trip); errors before and after the head; abort before and after the head reaches `ai_abort`; cancelling the body aborts too; events out of protocol order are refused.
- `runAgent`: the SDK's streamed beta tool runner over `tauriFetch`, with hand-written SSE fixtures in the format of the skill's `streaming.md`. Paths: a tool call; tool input that fails validation; approval declined; approval given; a `refusal`; a tool call in a turn that did not stop for `tool_use` (`max_tokens`, context window); the iteration limit; abort. Both requests of the tool-call test (the second carries the tool result) are checked against the Rust allowlists: every top-level body field and every `anthropic-beta` value the SDK actually sent is on the Rust lists.
- The dev-only page `#/ai-spike` (only when `import.meta.env.DEV`, like `#/bench`) is tested with the bindings and `runAgent` mocked: key stored, replaced and deleted; approval given and declined; Stop and leaving the page decline a pending approval and abort the request; a second approval in the same turn is declined; Run is disabled while running; a proxy refusal shows its reason.

**Key handling.** The SDK gets `apiKey: "injected-by-rust"` and `dangerouslyAllowBrowser: true`; Rust drops every incoming `x-api-key` and adds the stored one. The key field is a password input that is cleared before `ai_set_key` is called; the page only ever shows "Key stored: yes/no".

## Results

### Automated (no network)

| Suite | Result |
|---|---|
| `cargo xtask coverage` | `tf-ai-host` above the 80% adapter gate (`policy.rs` 100% lines, `body.rs` 90.5%) |
| `cargo test -p tf-ai-host` | 55 passed on Windows, 30 of them for the policy (7 new in Step 9.2: betas allowed, refused and trimmed; a bad beta refused before the body is read; body fields allowed and refused, also under JSON escapes and duplicates; only the default fallback) |
| `pnpm --filter @typefaced/ai test:ci` | 31 passed (`tauriFetch` 15, `runAgent` 10, tools 6); 97.8% statements, 94.7% branches; coverage above the 80% gate |
| `pnpm --filter @typefaced/desktop test:ci` | 52 passed, of which 12 for the AI spike page; coverage above the 80% gate |
| Bundle check: `! grep -rE 'sk-ant-[A-Za-z0-9_-]{16,}' apps/desktop/dist` | no match |

What the tests show against the ADR-0009 pass criteria:

- **Streaming in pieces through the proxy:** Rust emits each network read as a chunk before the body ends; `tauriFetch` resolves on the head and hands each chunk to the SDK as it arrives.
- **Key only in Credential Manager:** no command returns the key; the webview sends only the placeholder; the field is cleared before the IPC call; the built bundle has no key-shaped string.
- **Non-allowlisted destinations rejected:** from Step 9.1, now also betas and body fields.
- **Invalid tool input never runs:** the approval callback is never called and the font is unchanged; the model receives an `INVALID_JSON` error result.
- **The mutation tool needs approval:** declined leaves the font unchanged and returns "The user declined the change"; approved changes it.
- **Security review without open CRITICAL or HIGH findings:** four reviews (security, TypeScript, Rust, general). No CRITICAL. Two distinct HIGH issues, both fixed with tests before the PR: `tauriFetch` could close the body when the `ai_fetch` command resolved before the last chunks (TypeScript and general review; MEDIUM in the security review), and the spike page's approval promise could hang or outlive Stop (TypeScript review). Every finding and its resolution is listed in the PR.

### Live smoke test (GATE) — pending (manual)

To be filled in after the user runs it: date, prompt, whether the answer streamed, whether the approval dialog appeared, the family name afterwards, the ledger line (request ID, model, status, duration, byte counts; no body or key), and the cost.

### Debug-build CSP check — pending (manual)

To be filled in: the result of `fetch('https://api.anthropic.com/v1/models')` in the devtools console of the debug build (expected: refused by `connect-src`).

## Decision

Pending the two manual checks. If both pass, ADR-0009 becomes Accepted with the two accepted risks it now records (key replace or delete from the webview, closed by the M3 native credential prompt; the system proxy is honoured, for corporate networks).

## Follow-ups

- **Ajv under the CSP.** Ajv compiles validators with `new Function`, which the shipped CSP blocks. The spike page is dev-only and runs without the CSP; production code (M3) must use Ajv standalone (precompiled) validators.
- **Model ID.** ADR-0011 names `claude-opus-5` as the default; the current default is `claude-opus-5-5`. Amend ADR-0011 when the model configuration is built.
- **Fallback turns.** After a mid-stream fallback, the API asks that thinking and tool-use blocks before the last `fallback` block are not echoed back. The SDK 0.131.0 tool runner echoes the whole message; check this before the agent ships.
- **Unparseable tool JSON.** With eager input streaming, JSON the SDK cannot parse at all rejects the turn; the spike reports it as a failure. Production code should re-issue the turn (capped), as the `claude-api` skill describes.
- **Allowlists follow the client.** A new request feature (another beta, `tool_choice`, `metadata`, …) needs a change in `tf-ai-host` with a test, on purpose.
- **Policy refusals look like connection errors to the SDK.** A refusal before the head rejects `fetch`, so the SDK retries it twice and reports "Connection error." with the reason as its `cause` (the spike page shows it). Production code could send refusals as an HTTP-like 403 head instead.
- **Dev page loader.** `main.tsx` imports the dev pages without a `.catch`, as Step 8 did; fine for dev-only pages.
- **New commands for Step 12 to fold into `AGENTS.md`:** `pnpm --filter @typefaced/ai test`; the spike page at `#/ai-spike` under `pnpm --filter @typefaced/desktop tauri dev`.
