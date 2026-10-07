# ADR-0009: AI orchestration in TypeScript on the official SDK; Rust egress proxy + keychain
Status: Proposed · Date: 2026-09-29 · Amended: 2026-10-05 (OpenRouter)

Source: [implementation plan](../implementation-plan.md) §1 (D3), §3.4, §5.10, §6.2, §6.10, §11.1, §13.1; [M0 plan](../../plans/typefaced-m0-foundations-and-spikes.md) deviation 9, Steps 9.1 and 9.2.

## Context

- AI ships in 1.0 (D3). The agent needs a tool runner, streaming and context-management helpers (§3.4).
- An official Anthropic SDK exists for TypeScript, but not for Rust (§3.4).
- The UI runs in a webview. An XSS bug there must not be able to leak the API key (§6.2, §11.1).

## Decision

- **The agent runs in the webview** (`@typefaced/ai`) on the official `@anthropic-ai/sdk` tool runner (`client.beta.messages.toolRunner`; §6.2).
  - Tools are built from the command registry's JSON Schemas with `betaTool()` ([ADR-0004](0004-commands-as-single-api.md)).
  - `betaTool()` does not validate at runtime, so every tool's `run` function validates its input with Ajv against the same schema before dispatching it.
  - Approval gates live inside `run`. Tools with side-effect class E or D ask the user, and return a "user declined" result when refused.
- **The SDK's HTTP traffic goes through a Rust egress proxy** (§6.2). A custom `fetch` forwards each request over Tauri IPC to `tf-ai-host`, which:
  - forwards only to one allowlisted destination, and later the Typefaced gateway (§6.2). Since the amendment below, that is only `POST https://openrouter.ai/api/v1/messages` (query `beta=true` at most);
  - forwards only what the client actually sends: no `anthropic-beta` header at all, and only the top-level body fields `model`, `max_tokens`, `messages`, `system`, `tools`, `thinking`, `output_config`, `stream` and `fallbacks`. It refuses server-side tools (Anthropic's and OpenRouter's `openrouter:*`), `mcp_servers`, OpenRouter's request extensions (`plugins`, `provider`, `models`, `route`, …) and content sources the API would fetch from a URL or file (Steps 9.1 and 9.2). A new SDK feature therefore needs a deliberate change to the allowlists;
  - strips incoming `x-api-key`, `authorization` and `cookie` headers, and injects `Authorization: Bearer <key>` from the OS keychain (Step 9.1, amended);
  - streams the response back over a Tauri channel;
  - records usage in the budget ledger, writes the audit log, and enforces per-project AI permissions and a kill switch (§6.2);
  - never logs headers or bodies (Step 9.1).
- **The key lives only in the OS keychain** (Windows Credential Manager, through `keyring`), behind the `CredentialVault` port (§5.10). No command returns the key (Step 9.1).
- **`@tauri-apps/plugin-http` is not used:** it would need the key in JavaScript (Step 9.1).
- **An `LlmClient` port** (§6.2): OpenRouter's Anthropic-compatible Messages API, with the user's own OpenRouter key, in 1.0 (amendment below). A Typefaced Cloud gateway, if it is built, changes only the `baseURL` and the credential (§6.10).
- **SDK option names are never guessed.** Spike 4 confirmed them for `@anthropic-ai/sdk` 0.131.0: the client takes `baseURL` (`https://openrouter.ai/api`), `fetch` (the custom fetch), `apiKey` (a placeholder, `"injected-by-rust"`) and `dangerouslyAllowBrowser: true` (§6.2, §13.1; [spike report](../spikes/spike-4-ai-egress.md)).

### Amendment (2026-10-05): OpenRouter

User decision: the project uses an **OpenRouter API key**, not a direct Anthropic key.

- **Provider.** The SDK stays; its `baseURL` is `https://openrouter.ai/api`, so it calls OpenRouter's Anthropic-compatible `POST /api/v1/messages`. The egress policy allows only that endpoint on `https://openrouter.ai`; `api.anthropic.com` is now refused like any other host.
- **Credential.** The keychain entry is service `com.typefaced.desktop`, account `openrouter-api-key`. Rust injects it as `Authorization: Bearer <key>` (OpenRouter's scheme) after dropping any `x-api-key`, `authorization` or `cookie` from the webview.
- **Routing.** The app picks the model per task from a small constant table in `packages/ai` (`chat`, `agent`); each route names Claude models first, plus up to 3 OpenRouter `fallbacks` (`[{model}]`) for outages, rate limits and refusals ([ADR-0011](0011-model-configuration.md)). No auto router, no user picker. The policy allows only `anthropic/<model>` IDs without a `:variant` suffix (variants such as `:online` switch on OpenRouter features; other vendors and OpenRouter's own `openrouter/*` routers are refused), requires a model in every request, allows at most 3 fallbacks (each with only `model`), `max_tokens` up to 64000 and only adaptive `thinking`, so a request from the webview cannot pick a dearer model, budget or feature than the client sends.
- **Not used:** Anthropic's `fallbacks: "default"` and its beta, `eager_input_streaming` (not in OpenRouter's reference), `count_tokens` and `GET /models` (the client does not call them).
- **Data handling.** OpenRouter's `provider` options (`data_collection`, `zdr`, …) are refused for now; the account's privacy settings at openrouter.ai are the control (accepted risk 3).

### Key entry: a deliberate exception (deviation 9)

- §6.2 says the key never exists in the webview, so an XSS bug cannot leak it.
- In M0 the user types the key into a password field in the webview. It crosses IPC once, to `ai_set_key`, to be stored.
- The field is cleared right away. The key is never persisted, logged or readable back from JavaScript.
- **The cost:** while the key is being typed and sent, script running in the webview could read it. The webview XSS controls (strict CSP, no remote content, sanitised markdown, minimal Tauri capabilities; §11.1) lower this risk but do not remove it.
- **M3 evaluates a native OS credential prompt** instead, which would keep the key out of the webview entirely.

### Accepted risks

Three risks remain after Steps 9.1 and 9.2. The user accepted the first two in the review of Step 9.1 and the third with the OpenRouter decision (2026-10-05).

1. **Script in the webview can replace or delete the stored key without confirmation.** `ai_set_key` and `ai_delete_key` are callable from the main window, so an XSS bug could overwrite the key with another one (the user's requests would then run on someone else's account) or delete it. It cannot read the key, but a replaced key is also a data channel: the user's later prompts and font data would go to the other account, where its owner can read them in OpenRouter's activity logs. **Closed in M3** by the native OS credential prompt: key entry and deletion move out of the webview, and these two commands go away.
2. **The system proxy is honoured.** `tf-ai-host` uses the OS proxy settings, so a TLS-inspecting proxy whose root certificate the OS trusts (common on corporate networks) can see the key and the traffic. This is kept so that the app works on corporate networks, where such a proxy is often the only way out. The OS trust store is outside the app's control either way.
3. **Prompts, including font data, go to whichever upstream provider OpenRouter selects.** OpenRouter routes each request to one of the providers that serve the model (and to a fallback model when the first fails), and those providers have their own retention and training policies. The control is the user's OpenRouter account privacy settings (for example "no providers that train on prompts", or zero data retention); the app sends no `provider` preferences in M0.

## Consequences

- The SDK absorbs API changes and provides the tool runner, streaming and context-management helpers (§3.4).
- After entry, script in the webview cannot get the key back: no command returns it.
- Streaming crosses IPC. The JavaScript `fetch` resolves as soon as the response head arrives; the body follows in chunks (Step 9.1).
- The SDK needs `dangerouslyAllowBrowser: true` and a placeholder `apiKey` (`"injected-by-rust"`). That is acceptable only because Rust drops it and adds the real key as a bearer token (Step 9.2).
- The CSP is defence in depth. The real key control is the Rust proxy (Step 9.2). The debug-build CSP check shows only that the webview cannot reach the network directly; the AI path goes through Rust, which the CSP does not see, so the egress policy and its tests are the control there.
- Ajv compiles validators with `new Function`, which the shipped CSP (no `'unsafe-eval'`) blocks. The dev-only spike page runs under `tauri dev`, where Tauri applies no CSP; production code must use Ajv's precompiled (standalone) validators (Step 9.2).
- Every request goes through the allowlists, so adopting a new API feature (a beta, another body field, an OpenRouter extension) is a deliberate change to `tf-ai-host` with a test.
- OpenRouter is a second party between the app and the model: its availability, pricing and data handling now matter too (accepted risk 3).

## Alternatives considered

- **A Rust agent loop over raw HTTP.** Rejected: there is no official Rust SDK, and the TypeScript SDK absorbs API changes (§3.4). It is the fallback if the custom-fetch route cannot work, which needs the user's approval as an amendment to this ADR (Step 9.2).
- **`@tauri-apps/plugin-http`.** Rejected: it would need the key in JavaScript (Step 9.1).

## Validation

**Validated by Step 9.1 — Spike 4a: AI egress host (Rust), and Step 9.2 — Spike 4b: AI client and tool runner (TypeScript).**

Step 9.1 exit criteria:
- The policy, streaming and abort tests pass.
- The vault test passes on Windows and never touches the production entry.
- No command returns the key.
- The security review has no open CRITICAL or HIGH findings.

Step 9.2 exit criteria (the ADR-0009 pass criteria):
- Streaming responses arrive in pieces through the proxy.
- The key is stored only in Credential Manager. It is never persisted, logged or readable back in JavaScript, and it is absent from the built bundle.
- Non-allowlisted destinations are rejected (from Step 9.1).
- Invalid tool input never runs.
- The mutation tool needs approval.
- The security review has no open CRITICAL or HIGH findings.

Step 9.2 also runs a live smoke test (a GATE: the user enters the key and approves a rename) and a CSP check in a debug build. It then sets this ADR to Accepted or Rejected. Results: [spike report](../spikes/spike-4-ai-egress.md).
