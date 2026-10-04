# ADR-0013: UI stack: React + Zustand + imperative Canvas2D + WASM kernel
Status: Proposed · Date: 2026-09-29

Source: [implementation plan](../implementation-plan.md) §3.4, §4.2, §4.3, §5.2, §5.4, §7.1, §7.2, §7.3, §8.1, §10.4, §12.1, §12.2, §15 (R3); [M0 plan](../../plans/typefaced-m0-foundations-and-spikes.md) Invariants, Steps 2 and 8.

## Context

- Glyph editing needs a 60 fps drag on a 5,000-point glyph, and hit tests under 1 ms (§7.3, §10.4).
- Committed state lives in the Rust engine ([ADR-0002](0002-rust-engine-owns-document-state.md)), so a drag cannot wait for IPC on every frame.
- AI coding assistants know React best (§3.4).

## Decision

- **Stack** (§7.1):
  - TypeScript (strict), React, Vite and pnpm;
  - Zustand + Immer for the replica store, with memoised selectors;
  - TanStack Virtual for glyph grids and kerning tables; CodeMirror 6 for feature code;
  - Radix UI primitives + Tailwind CSS in the shadcn/ui style; harfbuzzjs for text shaping; i18next;
  - Vitest, Testing Library, Playwright and WebDriver E2E through `tauri-driver` for tests.
- **Canvas engine** (`@typefaced/canvas`, §7.3): imperative, retained-mode Canvas2D, independent of React. The package never imports React (§4.2).
  - Layers form a Composite; every tool is a typed finite-state machine.
  - A gesture ends in one committed command.
  - `Path2D` objects are cached by glyph id and revision.
  - Redraws are coalesced into one `requestAnimationFrame` per frame and triggered by events, never by a constant loop.
- **WASM kernel** (§5.2, §5.4): `tf-geometry`, plus `tf-interp` for the designspace sliders, compiled to `wasm32-unknown-unknown` through the `tf-wasm` facade. Calls are coarse-grained, on typed arrays (`hitTest`, `snap`, `constrainSmooth`, `fitStroke`, `interpolate`). Size budget: under 350 KB gzipped.
- **Dragging runs locally** in the webview on the WASM kernel. Only the final result is committed through IPC (§7.2, §8.1, Step 8).
- **Styling** uses CSS files bundled by Vite, not CSS-in-JS libraries that inject `<style>` tags. If inline styles are ever needed, `style-src 'self' 'unsafe-inline'` is added deliberately and noted in this ADR (Step 2).
- **The CSP's `script-src` includes `'wasm-unsafe-eval'`** for the WASM kernel (Step 8).

## Consequences

- The same Rust geometry code runs natively and in the UI (§4.3).
- The `wasm-pack` build becomes part of the build and of CI, and runs before the frontend build (Step 8, M0 plan Invariants).
- Tool state machines get unit tests of their transition tables, under the 80% TypeScript coverage target. Rendering code is covered by visual regression tests (Playwright + pixelmatch) instead of line coverage (§7.3, §12.1, §12.2).

## Alternatives considered

- **Svelte or Solid; a canvas rendered by React.** Rejected: AI coding assistants know React best, and the canvas must be imperative to reach 60 fps (§3.4).

## Validation

**Validated by Step 8 — Spike 3: IPC latency and WASM geometry kernel,** together with [ADR-0002](0002-rust-engine-owns-document-state.md). Its drag benchmark runs a synthetic 10 s pointer drag in which each frame calls the WASM hit-test and translate on a 5,000-point glyph and redraws it with Canvas2D `Path2D`. Step 8 validates the canvas and WASM parts of this ADR; React and Zustand are not benchmarked.

Pass criteria (Step 8 exit criteria):
- With the better payload format, the commit round-trip p95 is under 8 ms.
- Drag frame-time p95 is under 16 ms.
- A hit-test across all contours of the 5,000-point glyph takes under 1 ms.
- The numbers and the chosen payload format are recorded.
- If a budget fails, the report names the mitigation and the ADR is amended. It is never silently accepted.

Step 8 then sets this ADR to Accepted, or amends it, for example "move drag state further into the UI" (the R3 fallback).

## Amendment (Spike 3, 2026-10-03)

Report: [spike-3-ipc.md](../spikes/spike-3-ipc.md). Measured on one Windows laptop (i7-9750H, WebView2 with Chromium 154).

- **The WASM kernel passes its hit-test budget on a typical layout; the worst case is not shown.** A hit test over all 5,000 points in 50 contours with apart bounding boxes takes p95 0.3 to 0.8 ms in the WebView (0.07 to 0.23 ms native). That depends on layout: the hit test skips a contour when the pointer is outside its bounding box, which removes about 49 of 50 contours here and none for few large nested contours. On 2 × 2,500 and 5 × 1,000 concentric contours (5,000 points, pointer inside every box) it takes native p50 0.31 to 0.48 ms, p95 0.49 to 0.63 ms, and WASM under Node p50 0.47 ms, p95 0.53 to 1.00 ms. That is at the 1 ms budget before the WebView adds anything, so it may exceed it. The WebView number for these layouts is read from `hitTestWorstCase` in the next in-app run; until then this ADR does not claim the budget for them, and a spatial index per glyph revision is the follow-up if it fails. The module is 36.5 KiB raw and 16.3 KiB gzipped, far under 350 KB. The CSP needs `'wasm-unsafe-eval'`, which is set.
- **Open: the shipped CSP has not been exercised by a WebView run.** The benchmark runs against the dev server, where the CSP does not apply. wasm-bindgen's default `fetch()` of the module would be governed by `connect-src` (`ipc: http://ipc.localhost`, no `'self'`). Tauri's documentation says it adds nonces and hashes to script and style sources and is silent on `connect-src`, so no source was added to it. Instead the module is loaded from bytes inlined in the bundle (`kernel.ts`, `?inline`), which needs no network request. A test shows the load works without `fetch`; a run under the bundled app's CSP is still to do.
- **`Path2D` caching is required, not optional.** Rebuilding the paths of the 5,000-point glyph every frame costs 5 to 9 ms of script time at the median; cached paths cost about 0.5 ms. Draw handles only for the selected contours: drawing all 5,000 handle squares caused stalls in exploratory runs.
- **The 16 ms drag frame budget is not demonstrated.** An empty `requestAnimationFrame` loop in this window already runs at a 17.8 ms median. With cached paths and under 1.5 ms of script time per frame, frames still arrive every 35.7 ms in the benchmark page; the cause is outside the script and was not found. This ADR does not claim 60 fps until it is shown.
- **Status stays Proposed.** It becomes Accepted when the manual run in the spike report's follow-ups shows `drag.cached` frame time within 2 ms of the empty-frame cadence on hardware that reaches 16.7 ms, or when the budget is re-stated. No structural change is proposed: dragging stays local in the webview.

## Amendment (Spike 3 idle rerun and drawing investigation, 2026-10-04)

Report: [spike-3-ipc.md](../spikes/spike-3-ipc.md), section "Why the cached drag ran at 30 fps".

- **Hit-test budget: met in the WebView.** Idle, p95 is 0.4 ms (radius 8) and 0.3 ms (radius 1) on the grid glyph, and 0.5 to 0.6 ms on the worst case (2 × 2,500 and 5 × 1,000 nested contours, both radii; p99 0.6 to 0.8 ms). No spatial index is needed for M0.
- **The window reaches 60 Hz when idle.** Empty animation frames run at p50 16.6 to 16.7 ms, p95 16.7 to 16.8 ms; the 17.8 ms of 2026-10-03 came from machine load. The kernel-only drag (`compute`) holds that cadence with 0.6 ms of script per frame.
- **The cached drag as designed runs at 30 fps.** Frame time p50 and p95 33.3 to 33.4 ms with 0.5 to 0.8 ms of script. Cause, from DevTools traces: Canvas2D is GPU-accelerated (Intel UHD 630 through ANGLE/D3D11, Skia Ganesh), and every frame the GPU process fills again every filled path: the 50 outlines (10 to 13 ms) and one path of 5,000 handle squares (about 17 ms). Cached `Path2D` objects save script time only. The NVIDIA GPU, Skia Graphite, a software canvas, `alpha: false`, `desynchronized: true`, per-contour or combined paths and drawing without a transform do not fix it.
- **A layered frame reaches the display's cadence.** Drawing the still contours once per glyph revision into an offscreen bitmap, copying it each frame, and drawing vectors only for the dragged contours and their handles gave frame p95 16.8 ms against an empty-frame p95 of 16.8 ms, script under 2.5 ms and about 6 ms of GPU raster per frame (`dragProbes.stillBitmapSelectedHandles`). These runs were made with an agent session active.
- **Conditions proposed for acceptance** (they refine "`Path2D` objects are cached by glyph id and revision"): content that does not move during a gesture is drawn once per revision into a bitmap layer and copied each frame; vectors are drawn per frame only for what moves; point handles are drawn for the selection only.
- **Status stays Proposed.** It can be accepted with these conditions when an idle run shows `dragProbes.stillBitmapSelectedHandles.frameTime.p95` within 2 ms of `frameBaseline.p95`.
