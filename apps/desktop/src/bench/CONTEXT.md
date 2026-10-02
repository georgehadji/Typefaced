# apps/desktop/src/bench/

Dev-only benchmark page, shown at `#/bench` in development builds; production bundles
drop it. It measures IPC and WASM latency and reports the results to Rust. Up: [src/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `BenchPage.tsx` | The page: runs every benchmark once, logs progress, then sends the results to the `bench_report` command |
| `BenchPage.test.tsx` | Tests for `BenchPage` with the runners and Tauri mocked: reports results once, shows a failure |
| `runners.ts` | The measurements: commit round trip (JSON vs binary), patch stream over a `Channel`, WASM hit tests, synthetic drag. Needs the real WebView, so it is excluded from unit-test coverage |
| `outline.ts` | `PackedOutline` type, `makeGlyph` (synthetic glyph for the benchmarks), point flag constants and `decodeHit` for the WASM hit result |
| `outline.test.ts` | Tests for `makeGlyph` and `decodeHit` |
| `stats.ts` | `percentile` and `summarize`: the timing distribution (mean, min, p50, p95, p99, max) |
| `stats.test.ts` | Tests for `percentile` and `summarize` |
| `bench.css` | Styles for the page and its canvas |
