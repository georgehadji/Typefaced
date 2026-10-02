# .github/workflows/

GitHub Actions workflows. Up: [.github/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `ci.yml` | CI on every PR and on pushes to `main`. Job `windows` (windows-latest): install, frontend build, fmt, clippy, Rust tests, bindings are up to date, `pnpm lint && pnpm typecheck && pnpm test:ci`, `cargo xtask check-deps`, no API keys committed, tests do not write into the repository; on `main` it also builds the debug app. Job `gates` (ubuntu-latest): `cargo deny check`, `cargo xtask licenses-npm`, `cargo xtask context check` (the `CONTEXT.md` maps), `cargo xtask coverage`; it never builds the desktop crate |
