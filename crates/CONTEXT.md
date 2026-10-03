# crates/

Rust library crates of the engine. Every crate here is a member of the root workspace
(`crates/*`) and must declare `[package.metadata.typefaced] layer`. Domain crates are
pure: no I/O, no async runtime, no Tauri. Dependencies point inward only
(implementation plan §4.2; checked by `cargo xtask check-deps`). Up: [root CONTEXT.md](../CONTEXT.md).

| Subfolder | Layer | Context |
|---|---|---|
| `tf-cff/` | domain | [CONTEXT.md](tf-cff/CONTEXT.md): minimal CFF writer, cubic outlines to a `CFF ` table |
| `tf-commands/` | application | [CONTEXT.md](tf-commands/CONTEXT.md): typed command catalog shared by UI, AI, MCP and CLI |
| `tf-compile/` | adapter | [CONTEXT.md](tf-compile/CONTEXT.md): compiles UFO and designspace sources to TTF through fontc, in-process |

To add a crate: create `crates/<name>/Cargo.toml` with `edition`, `rust-version`,
`license` and `publish` set to `.workspace = true`, `[lints] workspace = true` and
`[package.metadata.typefaced] layer = "<layer>"`. Give it a `CONTEXT.md` (and one in
`src/`), start each Rust file with a `//!` comment, add a row above and to the root
folder map, then run `cargo xtask check-deps`, `cargo xtask context update` and
`cargo xtask context check`.
