# crates/tf-ai-host/src/

Up: [tf-ai-host/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `lib.rs` | Crate docs and the public API (re-exports from the modules below) |
| `vault.rs` | `CredentialVault`: one keychain entry for the API key. `production()` is service `com.typefaced.desktop`, account `anthropic-api-key` in Windows Credential Manager (`Local` persistence; `Unsupported` on other platforms). `set_key` accepts 1 to 512 visible ASCII characters; `has_key`, `delete_key` (no error when absent); `key()` is crate-private. Errors carry keyring's `Display` text only, never secret bytes. Tests use keyring-core's mock store, and on Windows a `com.typefaced.desktop.test` entry with a random account that a drop guard deletes |
| `policy.rs` | `ProxyRequest` (the IPC shape of a `fetch`) and `EgressPolicy`: only `https://api.anthropic.com` with path prefix `/v1/` (after URL normalisation; no user info), GET and POST only, no body on GET, bodies up to 32 MiB; passes only `anthropic-version`, `anthropic-beta`, `content-type` and `accept`, dropping every other header (`x-api-key`, `authorization`, `cookie` included) |
| `host.rs` | `EgressHost`: `fetch` checks the policy, adds `x-api-key` from the vault (marked sensitive), sends with reqwest (no redirects, connect and read timeouts) and emits `ProxyEvent`s: `Head` (status, headers without `set-cookie`), UTF-8 `Chunk`s as they arrive, then `End`, `Error` or `Aborted`. At most 16 requests in flight; request IDs are 1 to 64 of `[A-Za-z0-9_-]`. `abort(request_id)` cancels an in-flight request, or one that has not registered yet (it is then never sent). Writes one ledger line per sent request |
| `ledger.rs` | `UsageLedger`: appends one JSON line per request (API `request-id`, model, status, duration, byte counts) to a file; never bodies, headers or keys |
| `test_server.rs` | Test-only scripted HTTP/1.1 server on 127.0.0.1: records one request and answers with a chunked body whose parts can wait for the test, for the streaming and abort tests |
