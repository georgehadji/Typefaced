# crates/tf-commands/

Crate `tf-commands`, layer `application`: the typed command catalog, the one API
contract between the engine and every client (UI, AI, MCP, CLI). Every document change
is a command defined here (ADR-0004). Up: [crates/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `Cargo.toml` | Crate manifest: depends on `serde` and `specta` only; `serde_json` for tests |

| Subfolder | Context |
|---|---|
| `src/` | [CONTEXT.md](src/CONTEXT.md): command and payload types |
