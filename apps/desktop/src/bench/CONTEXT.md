# apps/desktop/src/bench/

Dev-only benchmark page, shown at `#/bench` in development builds; production bundles
drop it. It measures IPC and WASM latency and reports the results to Rust. Up: [src/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `BenchPage.tsx` | The page: runs every benchmark once (only the frame and drag ones when `VITE_BENCH_ONLY=drag`), then the drawing probes, logs progress (on screen and to the Rust log), then sends the results, or the error, to the `bench_report` command |
| `BenchPage.test.tsx` | Tests for `BenchPage` with the runners and Tauri mocked: reports results once, runs every drawing probe, drag-only mode, reports a failure to Rust, logs progress |
| `runners.ts` | The measurements: empty IPC round trip, commit round trip (JSON vs binary, 1 to 5,000 points), patch stream over a `Channel`, WASM hit tests (a grid glyph and the worst case, few large nested contours), empty animation frames, synthetic drag (compute only, rebuilt paths, cached paths, and the drawing probes of `draw.ts` on a fresh canvas). Needs the real WebView, so it is excluded from unit-test coverage |
| `draw.ts` | How a drag frame draws the glyph: `DrawSpec`, the `cached` settings, the drawing probes (`DRAG_PROBES`), `buildPaths`, `makeScene` (paths built once, still contours optionally in a bitmap) and `paintFrame` |
| `draw.test.ts` | Tests for `draw.ts` with a fake `Path2D` and a fake 2D context: paths per contour, handles, scene layouts, the calls of one frame |
| `outline.ts` | `PackedOutline` type, `makeGlyph` (synthetic glyph on a grid, the hit test's best case), `makeNestedGlyph` (concentric contours, its worst case), `innerBox`, point flag constants and `decodeHit` for the WASM hit result |
| `kernel.ts` | `loadKernel`: instantiates the WASM kernel once from bytes inlined in the bundle (`?inline`), so no `fetch` runs under the shipped CSP; `dataUrlBytes` decodes the data URL |
| `kernel.test.ts` | Tests for `dataUrlBytes` and `loadKernel` (loads the real module with `fetch` spied on, then runs a hit test) |
| `outline.test.ts` | Tests for `makeGlyph`, `makeNestedGlyph`, `innerBox` and `decodeHit` |
| `stats.ts` | `percentile` and `summarize`: the timing distribution (mean, min, p50, p95, p99, max) |
| `stats.test.ts` | Tests for `percentile` and `summarize` |
| `bench.css` | Styles for the page and its canvas |
