# xtask/

Crate `xtask`, layer `tool`: the repository task runner. Run with `cargo xtask <task>`
(alias in `.cargo/config.toml`). Tasks: `check-deps`, `licenses-npm`, `coverage`,
`corpus fetch`, `corpus verify`. Up: [root CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `Cargo.toml` | Crate manifest: `cargo_metadata`, `serde`, `serde_json`, `sha2`, `spdx`, `toml`, `walkdir` |

| Subfolder | Context |
|---|---|
| `src/` | [CONTEXT.md](src/CONTEXT.md): one module per gate |
