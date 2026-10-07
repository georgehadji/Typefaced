# crates/tf-ai-host/

Crate `tf-ai-host`, layer `adapter`: the Rust half of the in-app AI (ADR-0009,
implementation plan §6.2). It keeps the OpenRouter API key in the OS keychain and forwards
the webview SDK's requests to OpenRouter's Messages API with the key added, streaming the response
back. Nothing in its public API returns the key. Up: [crates/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `Cargo.toml` | Crate manifest: `keyring-core` (keychain API), `windows-native-keyring-store` on Windows only (no `search` feature), `reqwest` with the native TLS backend (default features off, so no rustls or aws-lc), `tokio` (`sync`, `macros`), `serde`, `serde_json`, `specta`, `thiserror`; more `tokio` features for the tests |

| Subfolder | Context |
|---|---|
| `src/` | [CONTEXT.md](src/CONTEXT.md): vault, egress policy, body check, egress host, usage ledger, test server |
