# crates/tf-commands/

Crate `tf-commands`, layer `application`: the typed command catalog, the one API
contract between the engine and every client (UI, AI, MCP, CLI). Every document change
is a command defined here (ADR-0004). Types and metadata only, no logic.
Up: [crates/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `Cargo.toml` | Crate manifest: `serde`, `serde_json`, `specta`, `schemars`, `thiserror` and `tf-geometry` (point flags); `insta` and `proptest` for tests |

| Subfolder | Context |
|---|---|
| `src/` | [CONTEXT.md](src/CONTEXT.md): catalog, registry, errors, packed codecs |
