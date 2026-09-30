# ADR-0001: Desktop shell: Tauri 2 + WebView2, Windows first
Status: Accepted · Date: 2026-09-29

Source: [implementation plan](../implementation-plan.md) §0, §1 (D5), §3.3, §5.18, §10.4, §11.1, §13.3, §15 (R11, R12), §17; [research](../research.md) §5, §9; [M0 plan](../../plans/typefaced-m0-foundations-and-spikes.md) Steps 2 and 3.2.

## Context

- Typefaced is a proprietary desktop font editor: a Rust engine with a TypeScript UI (§0).
- The research recommends Windows first. WebView2 is the same Chromium the UI is debugged in. Linux needs its WebKitGTK canvas performance tested first (research §9).
- The app needs a command-line sidecar (ttfautohint) and signed auto-updates (§3.3, §13.3).

## Decision

- The desktop shell is **Tauri 2** with **WebView2**.
- **Windows first** (D5). The targets are Windows 10 22H2+ and Windows 11, x64 first. A native ARM64 build follows once the Rust toolchain targets are set up (§13.3); whether it ships at launch is still open (§17, question 4). macOS and Linux come after 1.0.
- **One app process:** the Rust core plus the renderer processes that WebView2 manages. Short-lived command-line sidecars only, currently just ttfautohint. No Python at runtime (§3.3).
- **`apps/desktop/src-tauri` is the composition root** (§5.18). It:
  - wires adapters to ports;
  - keeps IPC handlers thin (deserialise → `EngineApi` → serialise);
  - forwards engine events to the UI over channels;
  - owns windows, menus, deep links, the updater, the Tauri capability configuration and a strict Content-Security-Policy.
- **Tauri-specific calls sit behind a small host-API layer**, so Electron stays possible as a fallback (R12, research §5).

## Consequences

- Installers stay small (a few MB), and Tauri provides sidecars and signed auto-updates (research §5).
- The webview is a security boundary. A strict CSP, no remote content and minimal Tauri capabilities are required controls (§11.1).
- Tauri applies the CSP only to the assets it serves itself, not to the Vite dev server, so CSP checks run in a debug build (Step 2).
- macOS and Linux wait until after 1.0, and until WebKit canvas performance is verified (§13.3, R12).
- Windows signing and SmartScreen are a risk. The mitigation is Microsoft Store MSIX plus a direct download signed with an OV certificate (R11, §13.3).

## Alternatives considered

From research §5:
- **Electron:** wins if Linux needs identical rendering (Tauri issue #5761: about 5 FPS canvas under WebKitGTK), but installers are 80–200 MB. It stays possible as a fallback through the host-API layer (R12).
- **Python + PySide6 (Qt):** the whole Python font toolchain in-process, but heavier packaging, slower UI work with AI assistance, and no built-in updater.
- **Flutter or Avalonia:** strong canvases, but no font tooling in Dart and little in .NET.
- **Rust-native GUIs** (egui, iced, Slint, Xilem, GPUI): maximum performance, but immature or tied to other projects.

## Validation

- Step 2 (walking skeleton) proves the shell on Windows: the running app shows a value that travels from Rust through the generated binding, and a debug build loads with the CSP active. From Step 3.2 on, the `windows` CI job compiles and tests the desktop crate on every PR.
- The §10.4 budgets, including cold start to a usable window under 1.5 s, run in CI as benchmarks with regression alerts.
- **Revisit** if WebView2 cannot meet the §10.4 budgets, or when macOS or Linux work starts and WebKit canvas performance falls short (R12).
