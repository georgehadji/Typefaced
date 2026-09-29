# Architecture decision records

New ADRs copy [template.md](template.md) and take the next free number. A spike changes its ADR from Proposed to Accepted or Rejected in the spike's own PR; the status lives only on each ADR's `Status:` line, so this index has no status column and parallel spikes never conflict on it.

In the ADRs, a bare § refers to the [implementation plan](../implementation-plan.md), "research §" to [research.md](../research.md), and "Step N" or "deviation N" to the [M0 plan](../../plans/typefaced-m0-foundations-and-spikes.md).

| ADR | Title | File |
|---|---|---|
| 0001 | Desktop shell: Tauri 2 + WebView2, Windows first | [0001-desktop-shell-tauri-2-webview2.md](0001-desktop-shell-tauri-2-webview2.md) |
| 0002 | Rust engine owns document state; the UI is a replica | [0002-rust-engine-owns-document-state.md](0002-rust-engine-owns-document-state.md) |
| 0003 | Persistent immutable state; snapshot undo; actor + read-copy-update | [0003-persistent-state-snapshot-undo-actor-rcu.md](0003-persistent-state-snapshot-undo-actor-rcu.md) |
| 0004 | Commands as the single API for UI, AI, MCP and CLI; generated types and schemas | [0004-commands-as-single-api.md](0004-commands-as-single-api.md) |
| 0005 | Native formats: UFO 3 + designspace 5, and the `.typefaced` package; `com.typefaced.*` lib keys | [0005-native-formats-ufo-designspace-typefaced-package.md](0005-native-formats-ufo-designspace-typefaced-package.md) |
| 0006 | Compiler: fontc in-process; fontmake as oracle and contingency | [0006-compiler-fontc-in-process.md](0006-compiler-fontc-in-process.md) |
| 0007 | CFF-based OTF via TTF → CFF transplant with `tf-cff` | [0007-cff-otf-via-ttf-transplant.md](0007-cff-otf-via-ttf-transplant.md) |
| 0008 | Boolean engine: skia-safe vs. linesweeper (from the spike) | [0008-boolean-engine.md](0008-boolean-engine.md) |
| 0009 | AI orchestration in TypeScript on the official SDK; Rust egress proxy + keychain | [0009-ai-orchestration-typescript-sdk-rust-egress.md](0009-ai-orchestration-typescript-sdk-rust-egress.md) |
| 0010 | AI edits as sandbox proposals with three-way merge | [0010-ai-edits-as-sandbox-proposals.md](0010-ai-edits-as-sandbox-proposals.md) |
| 0011 | Model configuration: `claude-opus-5` by default; config-driven; changes gated by evals | [0011-model-configuration.md](0011-model-configuration.md) |
| 0012 | MCP server via `rmcp`; off by default; sandboxed writes | [0012-mcp-server-rmcp.md](0012-mcp-server-rmcp.md) |
| 0013 | UI stack: React + Zustand + imperative Canvas2D + WASM kernel | [0013-ui-stack-react-zustand-canvas2d-wasm.md](0013-ui-stack-react-zustand-canvas2d-wasm.md) |
| 0014 | Studio and Workbench as workspace profiles over one engine | [0014-studio-and-workbench-workspace-profiles.md](0014-studio-and-workbench-workspace-profiles.md) |
| 0015 | Parametric glyph engine as the backbone of AI glyph generation | [0015-parametric-glyph-engine.md](0015-parametric-glyph-engine.md) |
| 0016 | Proprietary license, dependency policy and clean-room rule | [0016-proprietary-license-dependency-policy-clean-room.md](0016-proprietary-license-dependency-policy-clean-room.md) |
| 0017 | Persistent collections: `imbl` vs. an in-house chunked copy-on-write vector | [0017-persistent-collections.md](0017-persistent-collections.md) |
| 0018 | Hinting: ttfautohint sidecar for TTF only in 1.0 | [0018-hinting-ttfautohint-sidecar.md](0018-hinting-ttfautohint-sidecar.md) |
| 0019 | Clean-room spacing algorithm from the published method | [0019-clean-room-spacing-algorithm.md](0019-clean-room-spacing-algorithm.md) |
