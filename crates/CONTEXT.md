# crates/

Rust library crates of the engine. Every crate here is a member of the root workspace
(`crates/*`) and must declare `[package.metadata.typefaced] layer`. Domain crates are
pure: no I/O, no async runtime, no Tauri. Dependencies point inward only
(implementation plan §4.2; checked by `cargo xtask check-deps`). Up: [root CONTEXT.md](../CONTEXT.md).

| Subfolder | Layer | Context |
|---|---|---|
| `tf-commands/` | application | [CONTEXT.md](tf-commands/CONTEXT.md): typed command catalog shared by UI, AI, MCP and CLI |
