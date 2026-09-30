# Typefaced — agent instructions

Proprietary desktop font editor: Tauri 2 shell, Rust engine, React UI. All rights reserved.

## Read first
- Architecture: docs/implementation-plan.md · Decisions: docs/adr/ · Active plan: plans/ (status table at the top)

## Hard rules
- Clean room: never read, copy or paraphrase source code of GPL/AGPL/LGPL projects
  (Fontra, FontForge, Glyphr Studio, BirdFont, HT Letterspacer, potrace). Use specs,
  papers and permissively licensed code only.
- Dependencies: licenses must be on the allow-list in docs/implementation-plan.md
  Appendix B. Copyleft or unknown → stop and ask.
- Layers: every Rust crate declares `[package.metadata.typefaced] layer`. Domain crates
  are pure (no I/O, no async runtime, no Tauri). Dependencies point inward only
  (implementation plan §4.2).
- Every document change is a typed command (tf-commands). No back doors for UI, AI, MCP or CLI.
- Third-party APIs (fontc, fontations, Tauri, tauri-specta, @anthropic-ai/sdk, keyring…)
  change often: read the docs for the pinned version; never guess signatures or flags.
  For Claude API code use the claude-api skill.
- Secrets: never read, type, print or log API keys. Keys live only in the OS keychain.
- Product code is written test first; coverage gates are in `cargo xtask coverage`.
- Git: conventional commits; no attribution trailers; commit or push only when the user
  asks; never push to a public remote without the user's go-ahead.

## Commands
Toolchains are pinned: Rust in `rust-toolchain.toml`, Node in `.nvmrc`, pnpm in `package.json`.
The gates also need `cargo install --locked cargo-deny cargo-llvm-cov`.
Build the frontend before any cargo command: the desktop crate embeds `apps/desktop/dist`.

| Task | Command |
|---|---|
| Install | `pnpm install --frozen-lockfile` |
| Dev (app with hot reload) | `pnpm --filter @typefaced/desktop tauri dev` |
| Build the frontend | `pnpm --filter @typefaced/desktop build` |
| Build the app (debug, no installer) | `pnpm --filter @typefaced/desktop tauri build --debug --no-bundle` |
| Test | `cargo test --workspace` · `pnpm test` |
| Lint | `cargo fmt --all --check` · `cargo clippy --workspace --all-targets -- -D warnings` · `pnpm lint` (fix: `pnpm exec biome check --write .`) |
| Typecheck | `pnpm typecheck` |
| Regenerate the TypeScript bindings | `cargo test -p typefaced-desktop export_bindings`, then commit `packages/bindings/src/index.ts` |
| Layering and crate template | `cargo xtask check-deps` |
| Licenses (Rust · npm) | `cargo deny check` · `cargo xtask licenses-npm` (one allow-list: `deny.toml`) |
| Coverage gates (domain + application ≥ 90%, adapters ≥ 80%) | `cargo xtask coverage` |
