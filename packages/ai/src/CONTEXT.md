# packages/ai/src/

Up: [ai/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `index.ts` | The package's public API (re-exports from the files below) |
| `tauriFetch.ts` | `tauriFetch`: a `fetch` that calls `ai_fetch` with a fresh request ID and a channel. Resolves on the response head; the body is a stream fed by the later chunks. An error before the head rejects, after it errors the body. Aborting the signal or cancelling the body calls `ai_abort`. Takes only a URL and a string body |
| `agent.ts` | `createClaudeClient` (the SDK client: `baseURL` `https://openrouter.ai/api`, `fetch: tauriFetch`, a placeholder key, `dangerouslyAllowBrowser`), the `ROUTES` table (task `chat` or `agent` → OpenRouter model, up to 3 fallback models, effort; ADR-0011) and `runAgent`: one prompt through the SDK's streamed beta tool runner with the task's route, adaptive thinking and OpenRouter `fallbacks: [{model}]`. Stops on a `refusal`, or on a tool call in a turn that did not stop for `tool_use` (e.g. `max_tokens`), before that turn's tools run; reports hitting the iteration limit |
| `fontTools.ts` | The two demo tools on an in-memory font store: `get_font_summary` (read-only) and `set_family_name` (only after the approval callback says yes, else a "user declined" result). Both validate their input with Ajv before running; invalid input becomes an `INVALID_JSON` tool error |
| `fakeProxy.ts` | Test support, excluded from coverage: a fake of `ai_fetch` / `ai_abort` that replays scripted channel events (with pause points), and builders for Messages API SSE turns, wrapped as OpenRouter sends them (`: OPENROUTER PROCESSING` keep-alive, `data: [DONE]`) |
| `tauriFetch.test.ts` | Tests for `tauriFetch`: what is sent, resolving on the head, streaming, the body staying open until `end` when the command resolves first, errors before and after the head, abort before and after the head (from JS and from Rust), body cancel, protocol errors, refused inputs, null-body status |
| `agent.test.ts` | Tests for `runAgent` through `tauriFetch` with the IPC mocked and SSE fixtures replayed: request settings, routes and the egress rules (fields, model IDs, fallbacks, no beta), OpenRouter's stream comments and `[DONE]`, a tool call, invalid tool input, approval declined and given, refusal, truncated tool call, iteration limit, abort |
| `fontTools.test.ts` | Tests for the tools: definitions, schema validation, approval declined and given, store updates |
