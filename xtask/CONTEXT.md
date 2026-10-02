# xtask/

Crate `xtask`, layer `tool`: the repository task runner. Run with `cargo xtask <task>`
(alias in `.cargo/config.toml`). Tasks: `check-deps`, `licenses-npm`, `coverage`,
`corpus fetch`, `corpus verify`, `context check`, `context update`. Up: [root CONTEXT.md](../CONTEXT.md).

To add a task: put its pure logic and unit tests in a new module in `src/`, do the I/O
in `main.rs`, add it to `USAGE` and the dispatch `match`, then list it in `AGENTS.md`
(Commands) and, if CI must run it, in `.github/workflows/ci.yml`.

| File | What it does |
|---|---|
| `Cargo.toml` | Crate manifest: `cargo_metadata`, `serde`, `serde_json`, `sha2`, `spdx`, `toml`, `walkdir` |

| Subfolder | Context |
|---|---|
| `src/` | [CONTEXT.md](src/CONTEXT.md): one module per gate |
