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
| Hard rules, commands, CI | `AGENTS.md` (`CLAUDE.md` imports it) |
| Why a decision was made | `docs/adr/` |
| Full architecture and roadmap | `docs/implementation-plan.md` |
| Current work and what is left | `plans/` (status table) |
| IPC command types | `crates/tf-commands/src/lib.rs` → `packages/bindings/src/index.ts` |
| Tauri command registration | `apps/desktop/src-tauri/src/lib.rs` |
| Geometry kernel (hit testing, point edits) | `crates/tf-geometry/src/lib.rs` → `crates/tf-wasm/src/lib.rs` → `packages/geometry-wasm` |
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
| `apps/desktop/src/bench/` | [CONTEXT.md](apps/desktop/src/bench/CONTEXT.md) | Dev-only IPC and WASM benchmark page |
| `apps/desktop/src-tauri/` | [CONTEXT.md](apps/desktop/src-tauri/CONTEXT.md) | Tauri shell crate (driver layer); also covers `capabilities/` |
| `apps/desktop/src-tauri/icons/` | [CONTEXT.md](apps/desktop/src-tauri/icons/CONTEXT.md) | App icons for the bundle |
| `apps/desktop/src-tauri/src/` | [CONTEXT.md](apps/desktop/src-tauri/src/CONTEXT.md) | Tauri entry point and IPC registration |
| `crates/` | [CONTEXT.md](crates/CONTEXT.md) | Rust library crates of the engine |
| `crates/tf-commands/` | [CONTEXT.md](crates/tf-commands/CONTEXT.md) | Typed command catalog crate (application layer) |
| `crates/tf-commands/src/` | [CONTEXT.md](crates/tf-commands/src/CONTEXT.md) | Command and payload types |
| `crates/tf-geometry/` | [CONTEXT.md](crates/tf-geometry/CONTEXT.md) | Geometry kernel crate on packed outlines (domain layer) |
| `crates/tf-geometry/benches/` | [CONTEXT.md](crates/tf-geometry/benches/CONTEXT.md) | Headless timing of the hit test |
| `crates/tf-geometry/src/` | [CONTEXT.md](crates/tf-geometry/src/CONTEXT.md) | Hit testing and point edits |
| `crates/tf-wasm/` | [CONTEXT.md](crates/tf-wasm/CONTEXT.md) | WebAssembly facade crate over `tf-geometry` (driver layer) |
| `crates/tf-wasm/src/` | [CONTEXT.md](crates/tf-wasm/src/CONTEXT.md) | The exported WASM functions |
| `docs/` | [CONTEXT.md](docs/CONTEXT.md) | Research, implementation plan |
| `docs/adr/` | [CONTEXT.md](docs/adr/CONTEXT.md) | Architecture decision records |
| `docs/spikes/` | [CONTEXT.md](docs/spikes/CONTEXT.md) | Spike reports |
| `packages/` | [CONTEXT.md](packages/CONTEXT.md) | Shared TypeScript packages |
| `packages/bindings/` | [CONTEXT.md](packages/bindings/CONTEXT.md) | Generated IPC bindings package |
| `packages/bindings/src/` | [CONTEXT.md](packages/bindings/src/CONTEXT.md) | The generated `index.ts` |
| `packages/geometry-wasm/` | [CONTEXT.md](packages/geometry-wasm/CONTEXT.md) | Geometry kernel compiled to WebAssembly (`pkg/` is generated) |
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
| `AGENTS.md` | The instructions for every coding agent: read this file first, keep the maps true, hard rules, commands, CI jobs |
| `CLAUDE.md` | Imports `AGENTS.md` for Claude Code; holds nothing else |
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

Every file in the repository except the `CONTEXT.md` files. `cargo xtask context update`
generates this block; never edit it by hand. Not shown because git ignores them:
`target/`, `apps/desktop/dist/` and `packages/geometry-wasm/pkg/` (build output), `node_modules/`, `tests/corpus/.cache/`
(downloaded corpus) and `.claude/` (agent worktrees and local settings).

<!-- file-map:start (generated by `cargo xtask context update`) -->
```text
.
├── .cargo/config.toml
├── .github/
│   ├── workflows/ci.yml
│   └── pull_request_template.md
├── apps/desktop/
│   ├── src/
│   │   ├── bench/
│   │   │   ├── BenchPage.test.tsx
│   │   │   ├── BenchPage.tsx
│   │   │   ├── bench.css
│   │   │   ├── draw.test.ts
│   │   │   ├── draw.ts
│   │   │   ├── kernel.test.ts
│   │   │   ├── kernel.ts
│   │   │   ├── outline.test.ts
│   │   │   ├── outline.ts
│   │   │   ├── runners.ts
│   │   │   ├── stats.test.ts
│   │   │   └── stats.ts
│   │   ├── App.css
│   │   ├── App.test.tsx
│   │   ├── App.tsx
│   │   ├── main.tsx
│   │   └── vite-env.d.ts
│   ├── src-tauri/
│   │   ├── capabilities/default.json
│   │   ├── icons/
│   │   │   ├── 128x128.png
│   │   │   ├── 128x128@2x.png
│   │   │   ├── 32x32.png
│   │   │   ├── Square107x107Logo.png
│   │   │   ├── Square142x142Logo.png
│   │   │   ├── Square150x150Logo.png
│   │   │   ├── Square284x284Logo.png
│   │   │   ├── Square30x30Logo.png
│   │   │   ├── Square310x310Logo.png
│   │   │   ├── Square44x44Logo.png
│   │   │   ├── Square71x71Logo.png
│   │   │   ├── Square89x89Logo.png
│   │   │   ├── StoreLogo.png
│   │   │   ├── icon.icns
│   │   │   ├── icon.ico
│   │   │   └── icon.png
│   │   ├── src/
│   │   │   ├── bench.rs
│   │   │   ├── lib.rs
│   │   │   └── main.rs
│   │   ├── .gitignore
│   │   ├── Cargo.toml
│   │   ├── build.rs
│   │   ├── tauri.conf.json
│   │   └── windows-app-manifest.xml
│   ├── .gitignore
│   ├── index.html
│   ├── package.json
│   ├── tsconfig.json
│   ├── tsconfig.node.json
│   └── vite.config.ts
├── crates/
│   ├── tf-commands/
│   │   ├── src/lib.rs
│   │   └── Cargo.toml
│   ├── tf-geometry/
│   │   ├── benches/hit_test.rs
│   │   ├── src/lib.rs
│   │   └── Cargo.toml
│   └── tf-wasm/
│       ├── src/lib.rs
│       └── Cargo.toml
├── docs/
│   ├── adr/
│   │   ├── 0001-desktop-shell-tauri-2-webview2.md
│   │   ├── 0002-rust-engine-owns-document-state.md
│   │   ├── 0003-persistent-state-snapshot-undo-actor-rcu.md
│   │   ├── 0004-commands-as-single-api.md
│   │   ├── 0005-native-formats-ufo-designspace-typefaced-package.md
│   │   ├── 0006-compiler-fontc-in-process.md
│   │   ├── 0007-cff-otf-via-ttf-transplant.md
│   │   ├── 0008-boolean-engine.md
│   │   ├── 0009-ai-orchestration-typescript-sdk-rust-egress.md
│   │   ├── 0010-ai-edits-as-sandbox-proposals.md
│   │   ├── 0011-model-configuration.md
│   │   ├── 0012-mcp-server-rmcp.md
│   │   ├── 0013-ui-stack-react-zustand-canvas2d-wasm.md
│   │   ├── 0014-studio-and-workbench-workspace-profiles.md
│   │   ├── 0015-parametric-glyph-engine.md
│   │   ├── 0016-proprietary-license-dependency-policy-clean-room.md
│   │   ├── 0017-persistent-collections.md
│   │   ├── 0018-hinting-ttfautohint-sidecar.md
│   │   ├── 0019-clean-room-spacing-algorithm.md
│   │   ├── README.md
│   │   └── template.md
│   ├── spikes/
│   │   ├── spike-3-ipc.md
│   │   └── spike-6-collections.md
│   ├── implementation-plan.md
│   └── research.md
├── packages/
│   ├── bindings/
│   │   ├── src/index.ts
│   │   ├── package.json
│   │   └── tsconfig.json
│   └── geometry-wasm/
│       ├── .gitignore
│       └── package.json
├── plans/typefaced-m0-foundations-and-spikes.md
├── spikes/spike-collections/
│   ├── benches/
│   │   ├── memory.rs
│   │   └── ops.rs
│   ├── src/lib.rs
│   ├── Cargo.lock
│   └── Cargo.toml
├── tests/
│   ├── corpus/
│   │   ├── .gitignore
│   │   ├── SOURCES.md
│   │   └── manifest.toml
│   └── fixtures/min.ufo/
│       ├── glyphs/
│       │   ├── A_.glif
│       │   ├── A_acute.glif
│       │   ├── O_.glif
│       │   ├── _notdef.glif
│       │   ├── acute.glif
│       │   ├── contents.plist
│       │   └── space.glif
│       ├── fontinfo.plist
│       ├── layercontents.plist
│       └── metainfo.plist
├── xtask/
│   ├── src/
│   │   ├── context.rs
│   │   ├── corpus.rs
│   │   ├── coverage.rs
│   │   ├── deps.rs
│   │   ├── licenses.rs
│   │   └── main.rs
│   └── Cargo.toml
├── .editorconfig
├── .gitattributes
├── .gitignore
├── .nvmrc
├── AGENTS.md
├── CLAUDE.md
├── Cargo.lock
├── Cargo.toml
├── LICENSE
├── README.md
├── biome.json
├── clippy.toml
├── deny.toml
├── package.json
├── pnpm-lock.yaml
├── pnpm-workspace.yaml
└── rust-toolchain.toml
```
<!-- file-map:end -->

## Keeping these files true

`cargo xtask context check` runs in CI and fails when:
- a folder has no `CONTEXT.md`, or an exempt folder has one;
- a file is not named, in backticks, in its folder's `CONTEXT.md`;
- a subfolder is not listed in its parent's `CONTEXT.md`, or its `CONTEXT.md` is not
  linked from the folder map above;
- a relative link in a `CONTEXT.md` points to nothing;
- a Rust file does not start with a `//!` comment that says what it does;
- the file map above is out of date.

Keep facts that change often out of `CONTEXT.md`: step status lives in `plans/`, ADR
status on each ADR's `Status:` line, versions in the manifests.
