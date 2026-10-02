# Typefaced — project context

Start here. This file explains what the project is, maps every tracked file, and links
to the `CONTEXT.md` of every folder. Each folder's `CONTEXT.md` says what each of its
files does.

## What Typefaced is

A proprietary desktop font editor for Windows first. It has a guided **Studio** for
beginners and a **Workbench** for professionals, with AI built in. It reads and writes
UFO 3 + designspace 5 and exports TrueType (static and variable), CFF-based OpenType
and WOFF2.

- **Shell:** Tauri 2 + WebView2 (`apps/desktop/src-tauri`).
- **Engine:** Rust. The engine owns the document state; the UI is a replica (ADR-0002).
- **UI:** React + TypeScript (`apps/desktop/src`), built with Vite.
- **API:** every document change is a typed command (`crates/tf-commands`). Rust types
  are exported to TypeScript by tauri-specta into `packages/bindings` (generated, never
  edited by hand).
- **Layers:** each Rust crate declares a layer (`domain`, `application`, `adapter`,
  `driver`, `tool`) in `[package.metadata.typefaced]`. Dependencies point inward only;
  `cargo xtask check-deps` enforces this.

**Stage:** milestone M0 (foundations and technical spikes). The app only shows its name
and version over typed IPC. The status of each step is in the table at the top of
`plans/typefaced-m0-foundations-and-spikes.md`.

## Where to look for…

| Need | Go to |
|---|---|
| Hard rules, commands, CI | `CLAUDE.md` (also `AGENTS.md`) |
| Why a decision was made | `docs/adr/` |
| Full architecture and roadmap | `docs/implementation-plan.md` |
| Current work and what is left | `plans/` (status table) |
| IPC command types | `crates/tf-commands/src/lib.rs` → `packages/bindings/src/index.ts` |
| Tauri command registration | `apps/desktop/src-tauri/src/lib.rs` |
| UI code | `apps/desktop/src/` |
| Repository gates (layering, licenses, coverage, corpus) | `xtask/src/` |
| Test fonts | `tests/fixtures/` (committed), `tests/corpus/` (downloaded) |
| Spike code and results | `spikes/`, `docs/spikes/` |

## Folder map

| Folder | Context file | In one line |
|---|---|---|
| `/` | this file | Workspace manifests, toolchain pins, lint and license config |
| `.cargo/` | [CONTEXT.md](.cargo/CONTEXT.md) | Cargo alias for `cargo xtask` |
| `.github/` | [CONTEXT.md](.github/CONTEXT.md) | PR template |
| `.github/workflows/` | [CONTEXT.md](.github/workflows/CONTEXT.md) | CI pipeline |
| `apps/` | [CONTEXT.md](apps/CONTEXT.md) | Runnable applications |
| `apps/desktop/` | [CONTEXT.md](apps/desktop/CONTEXT.md) | Desktop app package: Vite, TypeScript and Vitest config |
| `apps/desktop/src/` | [CONTEXT.md](apps/desktop/src/CONTEXT.md) | React UI |
| `apps/desktop/src-tauri/` | [CONTEXT.md](apps/desktop/src-tauri/CONTEXT.md) | Tauri shell crate (driver layer); also covers `capabilities/` |
| `apps/desktop/src-tauri/icons/` | [CONTEXT.md](apps/desktop/src-tauri/icons/CONTEXT.md) | App icons for the bundle |
| `apps/desktop/src-tauri/src/` | [CONTEXT.md](apps/desktop/src-tauri/src/CONTEXT.md) | Tauri entry point and IPC registration |
| `crates/` | [CONTEXT.md](crates/CONTEXT.md) | Rust library crates of the engine |
| `crates/tf-commands/` | [CONTEXT.md](crates/tf-commands/CONTEXT.md) | Typed command catalog crate (application layer) |
| `crates/tf-commands/src/` | [CONTEXT.md](crates/tf-commands/src/CONTEXT.md) | Command and payload types |
| `docs/` | [CONTEXT.md](docs/CONTEXT.md) | Research, implementation plan |
| `docs/adr/` | [CONTEXT.md](docs/adr/CONTEXT.md) | Architecture decision records |
| `docs/spikes/` | [CONTEXT.md](docs/spikes/CONTEXT.md) | Spike reports |
| `packages/` | [CONTEXT.md](packages/CONTEXT.md) | Shared TypeScript packages |
| `packages/bindings/` | [CONTEXT.md](packages/bindings/CONTEXT.md) | Generated IPC bindings package |
| `packages/bindings/src/` | [CONTEXT.md](packages/bindings/src/CONTEXT.md) | The generated `index.ts` |
| `plans/` | [CONTEXT.md](plans/CONTEXT.md) | Milestone construction plans |
| `spikes/` | [CONTEXT.md](spikes/CONTEXT.md) | Throw-away experiments, each its own Cargo workspace |
| `spikes/spike-collections/` | [CONTEXT.md](spikes/spike-collections/CONTEXT.md) | Spike 6: persistent collections |
| `spikes/spike-collections/src/` | [CONTEXT.md](spikes/spike-collections/src/CONTEXT.md) | Candidate glyph tables |
| `spikes/spike-collections/benches/` | [CONTEXT.md](spikes/spike-collections/benches/CONTEXT.md) | Timing and memory benchmarks |
| `tests/` | [CONTEXT.md](tests/CONTEXT.md) | Shared test data |
| `tests/corpus/` | [CONTEXT.md](tests/corpus/CONTEXT.md) | Pinned, downloaded font corpus |
| `tests/fixtures/` | [CONTEXT.md](tests/fixtures/CONTEXT.md) | Small committed fonts; also covers `min.ufo/` |
| `xtask/` | [CONTEXT.md](xtask/CONTEXT.md) | Repository task runner crate (tool layer) |
| `xtask/src/` | [CONTEXT.md](xtask/src/CONTEXT.md) | Gate implementations |

Two folders have no `CONTEXT.md` on purpose, because tools read every file in them:
`apps/desktop/src-tauri/capabilities/` (Tauri capability files) and
`tests/fixtures/min.ufo/` (a UFO package). Their parent's `CONTEXT.md` describes them.

## Root files

| File | What it does |
|---|---|
| `AGENTS.md` | Instructions for any coding agent: read this file first; points to `CLAUDE.md` for the rules |
| `CLAUDE.md` | Agent instructions: hard rules, commands, CI jobs |
| `CONTEXT.md` | This file: project summary, folder map, file map |
| `README.md` | Short public description and status |
| `LICENSE` | Proprietary license, all rights reserved |
| `Cargo.toml` | Rust workspace: members (`crates/*`, the Tauri crate, `xtask`), shared package fields, shared dependency versions, lints (`unsafe_code`, `unwrap_used`, `expect_used` denied), release profile |
| `Cargo.lock` | Locked Rust dependency versions for the workspace |
| `rust-toolchain.toml` | Pins Rust 1.98.1 with rustfmt, clippy, llvm-tools and the `wasm32-unknown-unknown` target |
| `clippy.toml` | Allows `unwrap`/`expect` inside tests only |
| `deny.toml` | cargo-deny config; its `[licenses] allow` list is the single license allow-list for Rust and npm |
| `package.json` | pnpm workspace root: pinned pnpm, Biome and TypeScript; `lint`, `typecheck`, `test`, `test:ci` scripts run across packages |
| `pnpm-workspace.yaml` | Declares `apps/*` and `packages/*` as pnpm packages |
| `pnpm-lock.yaml` | Locked npm dependency versions |
| `biome.json` | Biome lint and format config; skips the generated bindings |
| `.nvmrc` | Pins Node 24 |
| `.editorconfig` | UTF-8, LF, 2-space indent (4 for Rust and TOML) |
| `.gitattributes` | LF line endings; marks fonts, images and archives as binary |
| `.gitignore` | Ignores build output, `node_modules`, `.env*`, coverage, Claude worktrees and local settings |

## File map

All tracked files. Folders marked † are not tracked (build output, caches, worktrees).

```
.
├── .cargo/config.toml
├── .github/
│   ├── pull_request_template.md
│   └── workflows/ci.yml
├── apps/desktop/
│   ├── .gitignore
│   ├── index.html
│   ├── package.json
│   ├── tsconfig.json
│   ├── tsconfig.node.json
│   ├── vite.config.ts
│   ├── dist/ †                       (frontend build, embedded by the Tauri crate)
│   ├── src/
│   │   ├── App.css
│   │   ├── App.test.tsx
│   │   ├── App.tsx
│   │   ├── main.tsx
│   │   └── vite-env.d.ts
│   └── src-tauri/
│       ├── .gitignore
│       ├── Cargo.toml
│       ├── build.rs
│       ├── tauri.conf.json
│       ├── windows-app-manifest.xml
│       ├── capabilities/default.json
│       ├── icons/                    (16 PNG, ICO and ICNS icons)
│       └── src/
│           ├── lib.rs
│           └── main.rs
├── crates/tf-commands/
│   ├── Cargo.toml
│   └── src/lib.rs
├── docs/
│   ├── implementation-plan.md
│   ├── research.md
│   ├── adr/
│   │   ├── README.md
│   │   ├── template.md
│   │   └── 0001-…md … 0019-…md      (19 ADRs)
│   └── spikes/spike-6-collections.md
├── packages/bindings/
│   ├── package.json
│   ├── tsconfig.json
│   └── src/index.ts                  (generated)
├── plans/typefaced-m0-foundations-and-spikes.md
├── spikes/spike-collections/
│   ├── Cargo.toml
│   ├── Cargo.lock
│   ├── src/lib.rs
│   └── benches/
│       ├── memory.rs
│       └── ops.rs
├── tests/
│   ├── corpus/
│   │   ├── .gitignore
│   │   ├── SOURCES.md
│   │   ├── manifest.toml
│   │   └── .cache/ †                 (downloaded corpus)
│   └── fixtures/min.ufo/
│       ├── fontinfo.plist
│       ├── layercontents.plist
│       ├── metainfo.plist
│       └── glyphs/
│           ├── contents.plist
│           ├── A_.glif
│           ├── A_acute.glif
│           ├── O_.glif
│           ├── _notdef.glif
│           ├── acute.glif
│           └── space.glif
├── xtask/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── corpus.rs
│       ├── coverage.rs
│       ├── deps.rs
│       └── licenses.rs
├── target/ †                         (Rust build output)
├── node_modules/ †
└── .claude/ †                        (agent worktrees and local settings)
```

## Keeping these files true

Any PR that adds, removes, renames or changes the role of a file updates the
`CONTEXT.md` of that folder, and this file when a folder is added or removed.
