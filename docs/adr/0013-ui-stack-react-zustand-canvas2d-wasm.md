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
