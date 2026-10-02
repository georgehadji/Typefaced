# docs/adr/

Architecture decision records. Each ADR's `Status:` line is the only source of its status
(a spike moves its ADR from Proposed to Accepted or Rejected). Up: [docs/CONTEXT.md](../CONTEXT.md).

To add an ADR: copy `template.md` to the next free number, add it to the index in
`README.md` and a row below, then run `cargo xtask context update` (root file map).

| File | What it does |
|---|---|
| `README.md` | How to add an ADR, how references (§, Step N) resolve, and the ADR index |
| `template.md` | Template for a new ADR |
| `0001-desktop-shell-tauri-2-webview2.md` | Desktop shell: Tauri 2 + WebView2, Windows first |
| `0002-rust-engine-owns-document-state.md` | The Rust engine owns document state; the UI is a replica |
| `0003-persistent-state-snapshot-undo-actor-rcu.md` | Persistent immutable state, snapshot undo, actor + read-copy-update |
| `0004-commands-as-single-api.md` | Commands are the single API for UI, AI, MCP and CLI; generated types and schemas |
| `0005-native-formats-ufo-designspace-typefaced-package.md` | Native formats: UFO 3 + designspace 5, the `.typefaced` package, `com.typefaced.*` lib keys |
| `0006-compiler-fontc-in-process.md` | Compiler: fontc in-process; fontmake as oracle and fallback (decided by Spike 1) |
| `0007-cff-otf-via-ttf-transplant.md` | CFF-based OTF via a TTF → CFF transplant with `tf-cff` (decided by Spike 2) |
| `0008-boolean-engine.md` | Boolean engine: skia-safe vs. linesweeper (decided by Spike 5) |
| `0009-ai-orchestration-typescript-sdk-rust-egress.md` | AI orchestration in TypeScript on the official SDK; Rust egress proxy and keychain (decided by Spike 4) |
| `0010-ai-edits-as-sandbox-proposals.md` | AI edits are sandbox proposals merged with a three-way merge |
| `0011-model-configuration.md` | Model configuration: config-driven default model; changes gated by evals |
| `0012-mcp-server-rmcp.md` | MCP server via `rmcp`; off by default; sandboxed writes |
| `0013-ui-stack-react-zustand-canvas2d-wasm.md` | UI stack: React + Zustand + imperative Canvas2D + WASM kernel (decided by Spike 3) |
| `0014-studio-and-workbench-workspace-profiles.md` | Studio and Workbench are workspace profiles over one engine |
| `0015-parametric-glyph-engine.md` | Parametric glyph engine as the backbone of AI glyph generation |
| `0016-proprietary-license-dependency-policy-clean-room.md` | Proprietary license, dependency license policy, clean-room rule |
| `0017-persistent-collections.md` | Persistent collections: `imbl` vs. an in-house chunked copy-on-write vector (decided by Spike 6) |
| `0018-hinting-ttfautohint-sidecar.md` | Hinting: ttfautohint as a sidecar, TTF only in 1.0 |
| `0019-clean-room-spacing-algorithm.md` | Clean-room spacing algorithm from the published method |
