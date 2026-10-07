# packages/ai/

Package `@typefaced/ai`: the Claude client of the webview (ADR-0009). It runs the
official Anthropic SDK against OpenRouter's Anthropic-compatible Messages API, with a
custom `fetch` that sends every request to the Rust egress
proxy (`ai_fetch` in `crates/tf-ai-host`), which adds the API key. The webview never
holds the key. The package exports `src/index.ts` directly (no build step). Up:
[packages/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `package.json` | Package manifest: `@anthropic-ai/sdk` and `ajv` (pinned exactly), `@tauri-apps/api`, the workspace bindings; Vitest with v8 coverage. Scripts: `typecheck`, `test`, `test:ci` (with coverage) |
| `tsconfig.json` | Strict TypeScript config for `src/` and `vitest.config.ts` (browser libs, no Node types) |
| `vitest.config.ts` | Vitest config: Node environment (the Tauri IPC is mocked), v8 coverage of `src/` with 80% thresholds; the tests and the test fake are excluded |

| Subfolder | Context |
|---|---|
| `src/` | [CONTEXT.md](src/CONTEXT.md): `tauriFetch`, the client and agent loop, the demo tools |
