# Spike 3: IPC latency and WASM geometry kernel

Tests [ADR-0002](../adr/0002-rust-engine-owns-document-state.md) and [ADR-0013](../adr/0013-ui-stack-react-zustand-canvas2d-wasm.md). Plan: [M0 Step 8](../../plans/typefaced-m0-foundations-and-spikes.md). Code: [`crates/tf-geometry`](../../crates/tf-geometry/), [`crates/tf-wasm`](../../crates/tf-wasm/), [`packages/geometry-wasm`](../../packages/geometry-wasm/), [`apps/desktop/src-tauri/src/bench.rs`](../../apps/desktop/src-tauri/src/bench.rs), [`apps/desktop/src/bench/`](../../apps/desktop/src/bench/).

**Verdict in one line.** On an idle machine the WASM hit test **meets** its 1 ms budget in the WebView, worst case included (p95 0.5 to 0.6 ms). The commit budget as written (round trip p95 under 8 ms) is **not met**, because WebView2's empty `invoke` alone takes about 6 ms at p50 and 7 to 9 ms at p95; a restated budget is proposed. The cached drag of ADR-0013 runs at **30 fps**, and the cause is found: the GPU process re-rasterises every filled path on every frame, and the full outline plus 5,000 handle squares cost 28 to 48 ms there. **Drawing the still contours once into a bitmap reaches the 60 Hz cadence** (p95 16.8 ms against an empty-frame p95 of 16.8 ms), confirmed by three idle runs (p95 16.8 to 16.9 ms against 16.8 to 16.9 ms). The user accepted the restated commit budget, and both ADRs are **Accepted** with amendments: ADR-0002 with the Rust-side commit budget, ADR-0013 with the layered drawing conditions.

## Question

Do the budgets of implementation plan §10.4 and §7.3 hold on this Windows machine, with the Rust engine owning the document and the UI dragging locally on a WASM geometry kernel?

- Commit round-trip (IPC + reducer + patch): p95 under 8 ms.
- Drag of a 5,000-point glyph at 60 fps: frame time p95 under 16 ms.
- Hit-test across all contours of the 5,000-point glyph: under 1 ms.
- Which payload format should the engine use, JSON or binary?

## Setup

| Item | Value |
|---|---|
| Machine | HP Pavilion Gaming Laptop 17-cd0xxx, Intel Core i7-9750H (6 cores / 12 threads, 2.6 GHz base), 32 GB RAM, Intel UHD Graphics 630 + NVIDIA GTX 1050 (hybrid; the display is driven by the Intel GPU), 1920×1080 at 60 Hz, "High performance" power plan |
| OS and WebView | Windows 11 Pro build 26300; WebView2 with Chromium 154 (`Edg/154.0.4258.53`) |
| Toolchain | rustc 1.98.1, cargo 1.98.1, Node 24.14.0, wasm-pack 0.15.0 |
| Crates | tauri 2.12.0, wasm-bindgen 0.2.129, kurbo 0.13.1, proptest 1 (tests only), serde_json 1 (`bench` feature only) |
| In-app build | `cargo build --release -p typefaced-desktop --features bench` (release profile: opt-level 3, LTO), run directly as `target\release\typefaced-desktop.exe` with `TYPEFACED_BENCH_EXIT=1`. Without the `custom-protocol` feature the binary loads the `devUrl`, so the frontend came from `pnpm --filter @typefaced/desktop dev` (Vite dev server, unminified JS). The `#/bench` route exists only in that dev frontend. The WASM package was the release build from `wasm-pack` (36.5 KiB raw, 16.3 KiB gzipped; the §5.4 budget is 350 KB gzipped) |
| Runs | **Primary: 2026-10-04, idle.** The user ran the in-app benchmark three times with the charger in and other programs closed, window visible and focused (`hasFocus: true`, `visible`). Tables give the range over the three runs. **Drawing investigation, 2026-10-04:** three drag-only runs and traced probes, made by an agent while its own session (and other agent sessions) were active; the empty-frame cadence moved between 16.6 and 18.0 ms with that load, so each probe is read against the empty-frame cadence of the same run. **Earlier: 2026-10-03, loaded** (see the end of *Results*) |

## Method

- **Glyph.** A synthetic glyph of 50 contours × 100 points = 5,000 points (`makeGlyph` in `outline.ts`, mirrored by `benches/hit_test.rs`). Each contour is a wavy ring: one line point, then cubic segments (off, off, curve). Points are about 3 font units apart. The packed layout is the §7.2 one: `Float64Array` coordinates, `Uint8Array` flags, `Uint32Array` contour ends.
- **Worst-case glyph.** The grid glyph is the best case for a hit test: its 50 contours have disjoint bounding boxes, so the per-contour box check skips about 49 of 50 contours. The worst case is a glyph of few large concentric contours with the pointer inside the box of every contour, so nothing is skipped. `makeNestedGlyph` builds two of them, both of 5,000 points: 2 contours × 2,500 points and 5 contours × 1,000 points. The 1,000 (in-app) or 2,000 (native) positions are uniform inside the innermost ring's bounding box.
- **Hit test.** `hit_test` finds the nearest point within the radius, otherwise the nearest segment (kurbo `nearest`), across all contours. Two radii: 8 units (mostly point hits) and 1 unit (finds segments).
- **Empty round trip.** 1,000 `invoke` calls of a command that returns an empty body, once handled on the main thread and once on the async runtime.
- **Commit round trip.** 1,000 calls (after 20 warm-up calls) per payload format and size. An edit of 1, 100, 1,000 or 5,000 points goes to Rust, which swaps in a new `Arc` and returns a patch of the same size. JSON: edit as a JSON array, patch as a JSON string. Binary: edit as a raw body of little-endian `f64`, patch as raw bytes (16-byte header, then coordinates). The timer covers building the payload, the call, and turning the answer into a `Float64Array`.
- **Patch stream.** Rust sends 600 binary patches at 16.67 ms over a `Channel` (10 s), once with 1-point patches and once with 5,000-point patches. Latency is measured from the send time stamped by Rust to the arrival in the page (same clock). "Late" means delivered more than one frame (16.67 ms) after sending.
- **Drag.** A synthetic 10 s pointer drag; each `requestAnimationFrame` moves the pointer on a circle, calls the WASM `hitTest` and `translatePoints` (1,000 of 5,000 points selected, the first 10 contours), then draws on a 760×760 Canvas2D. Three modes: `compute` (no drawing), `rebuild` (new `Path2D` objects for the whole glyph every frame: outline fill, stroke and 5,000 handle squares) and `cached` (`Path2D` objects built once; the 10 dragged contours are drawn with a canvas transform; this is the design in ADR-0013; it draws the handle squares of all 5,000 points). Frame time is the `requestAnimationFrame` delta; script work is the time spent in the frame callback. An empty `requestAnimationFrame` loop (3 s) gives the cadence of the window before any work.
- **Drawing probes.** 15 variants of `cached`, 5 s each, each on a fresh canvas (so context attributes apply): see *Why the cached drag ran at 30 fps*.
- **Headless kernel timings.** `cargo bench -p tf-geometry` (native, release) and the same WASM package under Node (V8), with the same glyphs and positions.

## Results

### Budgets against the measurements (idle runs, 2026-10-04)

| Criterion | Budget | Measured | Result |
|---|---|---|---|
| Hit-test, all contours of the 5,000-point glyph | under 1 ms | WebView p95: grid 0.4 ms (radius 8), 0.3 ms (radius 1); worst case (nested 2 × 2,500 and 5 × 1,000, both radii) 0.5 to 0.6 ms, p99 0.6 to 0.8 ms | **Met**, worst case included |
| Commit round trip, p95, better format (binary) | under 8 ms | 1 point 7.8 to 8.5 ms; 100 points 8.1 to 8.7 ms; 1,000 points 10.8 to 11.6 ms; 5,000 points 17.0 to 17.3 ms. An empty command: p50 5.6 to 6.2 ms, p95 7.1 to 9.0 ms | **Not met as written.** The floor of one `invoke` already reaches the budget; restated and accepted (*Decision* 3) |
| Drag frame time, p95 | under 16 ms (read as: within 2 ms of the empty-frame cadence, which is 16.7 ms at 60 Hz) | Empty frames p95 16.7 to 16.8 ms. `cached` p95 33.4 ms (every frame misses one vsync); `rebuild` p95 50.0 ms. In the drawing investigation, still contours drawn once into a bitmap: p95 16.8 ms | **Not met by `cached` as designed. Met by the bitmap variant**, confirmed idle (p95 16.8 to 16.9 ms) |
| Numbers and chosen payload format recorded | | This report; binary | Done |

### Empty round trip (ms)

| Handler | p50 | p95 | p99 |
|---|---|---|---|
| Main thread | 5.6 to 6.2 | 7.1 to 9.0 | 8.9 to 11.8 |
| Async runtime | 5.9 to 6.8 | 7.8 to 11.1 | 8.9 to 18.2 |

An `invoke` that does nothing costs about 6 ms at the median and 7 to 9 ms at p95 on the idle machine. That is the floor for every command. The thread hop of the async runtime adds under 1 ms at p50.

### Commit round trip (ms, range over three runs)

| Points | Payload (JSON / binary) | JSON p50 | JSON p95 | Binary p50 | Binary p95 | Binary p99 |
|---|---|---|---|---|---|---|
| 1 | 28 B / 16 B | 6.2 to 6.5 | 8.0 to 8.5 | 6.2 to 6.5 | 7.8 to 8.5 | 8.8 to 10.0 |
| 100 | 3.6 KB / 1.6 KB | 7.2 to 7.8 | 8.9 to 10.7 | 6.3 to 6.6 | 8.1 to 8.7 | 9.0 to 9.8 |
| 1,000 | 35.7 KB / 16 KB | 13.3 to 13.7 | 15.6 to 17.0 | 8.0 to 8.4 | 10.8 to 11.6 | 13.7 to 15.4 |
| 5,000 | 177 KB / 80 KB | 42.8 to 44.0 | 46.0 to 47.6 | 14.6 to 14.8 | 17.0 to 17.3 | 19.6 to 20.2 |

- A 1-point or 100-point binary commit costs the same as an empty command: the reducer (swap an `Arc`) does not show.
- Binary is 1.5× faster than JSON at 1,000 points and 2.7× faster at 5,000 points (p95).
- These are page-side round trips; the Rust side was not timed separately.

### Patch stream (600 patches at 60 Hz)

| Patch | Delivery latency p50 | p95 | Late (over 16.7 ms), of 600 | Inter-arrival p95 |
|---|---|---|---|---|
| 1 point | 1.1 to 1.2 ms | 1.5 to 1.6 ms | 2, 3, 2 | 17.2 ms |
| 5,000 points | 10.6 to 10.8 ms | 12.5 to 13.0 ms | 5, 5, 4 | 17.9 to 18.1 ms |

Small patches arrive in about 1.5 ms. Whole-glyph patches at 60 Hz arrive in 11 ms at the median and are late under 1% of the time when idle. Patches should still scale with the change, not with the glyph.

### Hit test in the WebView (ms per call, 1,000 calls)

| Layout | Radius | p50 | p95 | p99 | Mean of the batch |
|---|---|---|---|---|---|
| Grid 50 × 100 (best case) | 8 | 0.1 | 0.4 | 0.5 | 0.17 |
| Grid 50 × 100 | 1 | 0.2 | 0.3 | 0.3 | 0.18 to 0.19 |
| Nested 2 × 2,500 (worst case) | 8 | 0.4 to 0.5 | 0.5 to 0.6 | 0.6 to 0.7 | 0.44 to 0.46 |
| Nested 2 × 2,500 | 1 | 0.5 | 0.5 to 0.6 | 0.6 to 0.8 | 0.46 to 0.47 |
| Nested 5 × 1,000 (worst case) | 8 | 0.4 | 0.5 to 0.6 | 0.6 to 0.8 | 0.36 to 0.37 |
| Nested 5 × 1,000 | 1 | 0.5 | 0.5 to 0.6 | 0.6 to 0.7 | 0.44 to 0.45 |

`performance.now()` has 0.1 ms resolution in the WebView, so the per-call percentiles are coarse; the batch mean is finer. In each run one grid call (radius 8, the first set measured after the kernel loads) took 6.7 to 7.0 ms; p99 is 0.5 ms, so it is a single outlier. Its cause was not checked. The worst case is about 2.5 times the grid's batch mean and stays under half the budget at p95.

### Hit test and translate, headless

| Case | p50 | p95 | p99 |
|---|---|---|---|
| `hit_test`, grid 50 × 100 (best case), radius 8 / 1, native (3 runs) | 0.078 to 0.084 ms | 0.099 to 0.172 ms | 0.145 to 0.347 ms |
| `hit_test`, nested 2 × 2,500 (worst case), radius 8 / 1, native (3 runs) | 0.32 to 0.48 ms | 0.51 to 0.62 ms | 0.63 to 0.99 ms |
| `hit_test`, nested 5 × 1,000 (worst case), radius 8 / 1, native (3 runs) | 0.31 to 0.43 ms | 0.49 to 0.63 ms | 0.58 to 0.88 ms |
| `translate_points`, 1,000 of 5,000 selected, native | 0.23 to 0.25 ms | 0.32 to 0.41 ms | 0.36 to 0.53 ms |
| WASM `hitTest` under Node, grid, radius 8 / 1 (3 runs) | 0.19 to 0.22 ms | 0.30 to 0.38 ms | 0.36 to 0.54 ms |
| WASM `hitTest` under Node, nested 2 × 2,500, radius 8 / 1 (3 runs) | 0.47 to 0.48 ms | 0.79 to 1.00 ms | 0.89 to 1.56 ms |
| WASM `hitTest` under Node, nested 5 × 1,000, radius 8 / 1 (3 runs) | 0.47 to 0.48 ms | 0.53 to 0.94 ms | 0.73 to 1.23 ms |
| WASM `translatePoints` under Node | 0.22 ms | 0.33 to 0.34 ms | 0.45 to 0.70 ms |

These were measured on 2026-10-03 while other agent sessions kept the machine busy (CPU counter 8% to 100%), so their tails are pessimistic. The idle WebView numbers above are lower than the loaded Node numbers. Two changes made the hit test fast: visiting segments without allocating (the first version built a `Vec` of all segments per call, p50 0.42 to 0.53 ms), and skipping contours whose bounding box is out of reach (grid p50 0.055 ms natively). Box culling does nothing for nested contours, which is why the worst case costs three to five times more. Property tests compare `hit_test` with a brute-force search.

### Drag (760×760 canvas, 10 s each, idle)

| Mode | Frame p50 | Frame p95 | Frames over 25 ms | Script work p50 | Script work p95 |
|---|---|---|---|---|---|
| Empty animation frames (3 s) | 16.6 to 16.7 ms | 16.7 to 16.8 ms | | | |
| `compute` (hit test + translate, no drawing) | 16.7 ms | 16.7 to 16.8 ms | 0 of 601 | 0.3 to 0.4 ms | 0.6 ms |
| `rebuild` (new paths every frame) | 33.3 ms | 50.0 ms | 274 to 283 of about 285 | 4.9 to 5.1 ms | 6.0 to 8.2 ms |
| `cached` (paths built once) | 33.3 ms | 33.4 ms | 289 to 291 of about 305 | 0.5 ms | 0.7 to 0.8 ms |

- **The kernel is cheap.** With drawing off the drag runs at the display's cadence and script time is under 1 ms.
- **Caching the paths is needed.** Rebuilding them costs 5 ms of script time per frame; cached paths cost 0.5 ms.
- **Idle, the window reaches 60 Hz.** The empty loop runs at 16.7 ms, so the 17.8 ms cadence of 2026-10-03 came from the machine load.
- **`cached` runs at a steady 30 fps** although its script time is under 1 ms. The next section finds why.

### Why the cached drag ran at 30 fps

**Method.**
1. GPU facts: the app ran with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`; a Node script read `SystemInfo.getInfo` over the DevTools protocol (browser target).
2. Probes: `apps/desktop/src/bench/draw.ts` defines 15 variants of the `cached` frame (`DRAG_PROBES`). The page runs each for 5 s after the three drag modes and stores them under `dragProbes` in the results JSON, with the settings used. `VITE_BENCH_ONLY=drag`, set when starting the dev server, runs only the frame and drag benchmarks (about 2 minutes). Three drag-only runs were made.
3. Traces: the same probe code was run through the debugging port inside a `Tracing` session (categories `devtools.timeline`, `gpu`, `viz`, `cc`, `blink`, `toplevel`, `benchmark`), 3 s per variant, and the per-frame duration of `RasterDecoderImpl::DoRasterCHROMIUM` on the GPU process main thread was read. That event is the GPU process playing back the canvas's draw commands for one frame. One extra trace with the Skia categories showed the time of each draw call. Tracing slows the GPU process a little, so the traced frame times are not used; the traced raster times are.
4. The Skia per-call trace, the GPU switch and the Skia backend switch below used a throwaway page with the same drawing code as `draw.ts`, not the bench page.
5. GPU choice: the Chromium switch `--force_high_performance_gpu` (added to the same environment variable, so it applies to this app only) moved the WebView2 GPU process to the NVIDIA GPU. The Skia backend was switched with `--enable-skia-graphite`. No system setting was changed.

**GPU facts** (`SystemInfo.getInfo`, default launch):

| Item | Value |
|---|---|
| Feature status | `2d_canvas: enabled`, `rasterization: enabled`, `gpu_compositing: enabled`, `multiple_raster_threads: enabled_on`, `skia_graphite: disabled_off` |
| GPU in use | Intel UHD Graphics 630 (driver 31.0.101.2145) through ANGLE on Direct3D 11 (`ANGLE (Intel, Intel(R) UHD Graphics 630 … Direct3D11 …)`); the GPU process is out of process and sandboxed |
| Skia backend | Ganesh on GL (`skiaBackendType: GaneshGL`) |
| Driver bug workarounds that touch drawing | `msaa_is_slow`, `msaa_is_slow_2`, `max_msaa_sample_count_4` (the rest are video encode/decode and overlay workarounds) |
| NVIDIA GTX 1050 | Listed (driver 32.0.15.8183) but not used by default; it drives no display |

So Canvas2D is GPU-accelerated, on the Intel GPU. Its draw commands are recorded in the page, serialised, and played back by Skia in the GPU process.

**Per-variant numbers.** Frame time from the three drag-only runs (run 1 / run 2 / run 3; the empty-frame p95 of those runs was 18.3 / 16.8 / 16.8 ms, so 18 ms in run 1 is the cadence of a loaded machine, not a miss). Raster time is the median `DoRasterCHROMIUM` per frame in one traced 3 s run.

| Variant | What changes against `cached` | Frame p50 (ms) | Frame p95 (ms) | Script p95 (ms) | GPU raster p50 (ms) |
|---|---|---|---|---|---|
| empty frames | no drawing, no kernel | 18.0 / 16.6 / 16.6 | 18.3 / 16.8 / 16.8 | | |
| `compute` | no drawing | 18.0 / 16.6 / 16.7 | 18.3 / 16.8 / 16.8 | 0.6 to 1.2 | 0.1 |
| `rebuild` | paths rebuilt every frame | 35.7 / 50.0 / 66.4 | 53.2 / 66.8 / 66.8 | 8.5 to 9.9 | 50.8 |
| `cached` | — | 35.7 / 49.9 / 49.9 | 52.9 / 66.6 / 66.4 | 0.8 to 1.1 | 48.1 |
| `control` | nothing (fresh canvas) | 35.9 / 50.0 / 49.8 | 54.1 / 66.6 / 50.0 | 0.8 to 1.3 | 40.3 |
| `strokeOnly` | outline stroke only, no fill, no handles | 17.9 / 16.7 / 16.7 | 18.3 / 16.8 / 16.8 | 0.7 to 1.1 | 0.4 |
| `fillOnly` | outline fill only, no handles | 17.9 / 16.7 / 16.7 | 18.3 / 33.4 / 33.4 | 0.7 to 1.0 | 13.3 |
| `outlineNoHandles` | fill + stroke, no handles | 17.9 / 16.7 / 16.7 | 18.3 / 33.4 / 33.4 | 0.6 to 1.0 | 11.9 |
| `handlesOnly` | all 5,000 handles, no outline | 18.0 / 33.3 / 33.3 | 36.0 / 33.5 / 33.7 | 0.7 to 0.9 | 19.4 |
| `selectedHandles` | handles of the 10 dragged contours only | 17.9 / 16.7 / 16.7 | 18.7 / 33.4 / 33.4 | 0.7 to 1.1 | 14.8 |
| `canvas380` | 380×380 canvas | 18.3 / 33.3 / 33.3 | 36.3 / 50.0 / 50.0 | 0.7 to 1.0 | 21.9 |
| `desynchronized` | `getContext("2d", { desynchronized: true })` | 35.3 / 16.8 / 16.9 | 54.8 / 99.9 / 99.9 | 0.7 to 0.9 | 31.3 |
| `alphaFalse` | `{ alpha: false }` | 53.6 / 49.9 / 50.0 | 71.5 / 66.5 / 67.4 | 1.0 to 1.2 | 29.8 |
| `softwareCanvas` | `{ willReadFrequently: true }` (canvas rasterised on the CPU in the page) | 36.1 / 33.3 / 49.9 | 53.9 / 49.9 / 66.7 | 0.7 to 0.9 | 0.1 (see below) |
| `perContourPaths` | one `Path2D` per contour (50 + 50 draws) | 53.8 / 99.9 / 66.7 | 109.3 / 116.7 / 100.0 | 1.1 to 1.2 | 23.7, plus 44 ms in the flush |
| `combinedPath` | one `Path2D` for the whole glyph, nothing moves | 53.6 / 50.0 / 33.3 | 89.0 / 66.6 / 50.0 | 1.0 to 1.1 | 29.6 |
| `noTransform` | dragged contours drawn without `translate` | 36.5 / 49.9 / 33.3 | 54.2 / 66.6 / 50.0 | 0.9 to 1.2 | 33.5 |
| `stillBitmap` | still contours (and their handles) drawn once into a bitmap; each frame copies it, then draws the dragged contours | 18.0 / 16.6 / 16.6 | 18.4 / 16.8 / 16.8 | 1.0 to 1.1 | 7.2 |
| **`stillBitmapSelectedHandles`** | `stillBitmap` with handles of the dragged contours only | **16.7 / 16.7 / 16.7** | **16.8 / 16.8 / 16.8** | 1.1 to 2.5 | **5.9** |

`willReadFrequently: false` is the default, so it is the `control` probe. Frames over 25 ms for the last two probes: `stillBitmap` 2, 4 and 2 of about 290; `stillBitmapSelectedHandles` 4, 0 and 0 of 243 to 301. Their maximum frames were 50 to 126 ms (`stillBitmap`) and, in run 1 only, 765 ms (`stillBitmapSelectedHandles`; runs 2 and 3: 17.7 and 16.9 ms). The one-time raster of the bitmap at the first frame is a likely source of the 50 to 126 ms frames; it was not checked. Frame counts per probe are in `dragProbes.<name>.frames`.

**What the traces show.**
- The frame callback takes under 1 ms, then the GPU process main thread spends the time in `DoRasterCHROMIUM`: 28.5 to 29.6 ms per frame for `cached` in the first traced sessions (throwaway page), 48 ms in the traced run of the bench code (the machine was busier then). Over 16.7 ms means the frame misses a vsync; over 33.3 ms, two. That matches the 33.3 ms cadence of the idle runs and the 50 ms cadence under load.
- Per draw call (Skia trace, one frame of `cached`): fill of the 40 still outlines 8.1 to 9.9 ms; their stroke 0.1 to 0.2 ms; fill of the 4,000 still handle squares (one path) 13.0 to 14.7 ms; fill of the 10 dragged outlines 2.1 to 2.8 ms; their stroke 0.1 ms; fill of the 1,000 dragged handle squares 3.1 to 3.5 ms. Each frame also has about four `GrGpu::writePixels` calls, one per filled path.
- Strokes are cheap (one-pixel hairlines on the GPU). **Fills are expensive and scale with the number of edges**: the outline fill costs 10 to 13 ms for 50 contours, and one path of 5,000 small squares costs about 17 ms.
- Nothing is reused between frames. The page keeps its `Path2D` objects, but every frame's draw commands are serialised again and the GPU process fills each path again. A 380×380 canvas cuts raster time by about a fifth (22.2 ms against 28.5 ms in the same session), so the cost depends on the path more than on the pixel count.
- **The GPU is not the bottleneck.** On the NVIDIA GTX 1050 (`--force_high_performance_gpu`, confirmed by `SystemInfo`: `ANGLE (NVIDIA, NVIDIA GeForce GTX 1050 …)`, no MSAA workarounds) `cached` took 29.2 ms of raster per frame, the same as on the Intel GPU (28.5 to 29.6 ms), and the frame cadence was the same (35.8 ms at p50 with that session's 18 ms empty-frame cadence). With Skia Graphite (`--enable-skia-graphite`, confirmed `GraphiteDawnD3D11`) `cached` also ran at 35.7 ms p50 and `selectedHandles` got worse (35.8 ms). Moving the canvas to the CPU (`softwareCanvas`) moved the same cost to the page's main thread (`Paint`, 29 ms per frame in a separate trace) and did not help.
- The `writePixels` per fill and the equal cost on both GPUs and in the software canvas are consistent with antialiased path fills being rasterised as coverage masks on the CPU and uploaded. This was not checked against Skia's source.

**Cause.** Every frame re-fills, in the GPU process, every filled path on the canvas: the outlines of all 50 contours and one path of 5,000 handle squares. That costs 28 to 48 ms per frame on the GPU process main thread on this machine, about the same on either GPU, so frames come every second (idle) or third (loaded) vsync. Script time, the WASM kernel, the canvas transform, the context attributes, the GPU model (Intel or NVIDIA) and the Skia backend (Ganesh or Graphite) are ruled out.

**What reaches 60 fps.** `stillBitmapSelectedHandles`: draw the still contours once per glyph revision into an offscreen canvas of the same size; each frame, copy that bitmap with `drawImage` (device pixels, identity transform), then draw only the dragged contours (fill, stroke) and their handle squares through the transform. Raster time drops to about 6 ms per frame and the cadence equals the empty-frame cadence (p95 16.8 ms in the two runs whose empty-frame p95 was 16.8 ms). `stillBitmap` (all handles, the still ones inside the bitmap) also holds the cadence. Both were measured with agent load on the machine; the idle confirmation below repeats them.

**Idle confirmation (2026-10-04).** Three drag-only runs (`VITE_BENCH_ONLY=drag`) by the user on the idle machine: charger in, other programs closed, window in front, the same release binary. Frame time in ms; empty-frame p95 per run is the reference.

| Run | Empty frames p95 | `stillBitmapSelectedHandles` p50 / p95 / max | `stillBitmap` p50 / p95 / max | `cached` p50 / p95 | `compute` p95 |
|---|---|---|---|---|---|
| 1 | 16.9 | 16.7 / 16.8 / 20 | 16.7 / 16.8 / 100 | 33.3 / 50.1 | 16.8 |
| 2 | 16.8 | 16.7 / 16.9 / 33 | 16.7 / 16.8 / 83 | 49.9 / 66.7 | 16.8 |
| 3 | 16.8 | 16.7 / 16.8 / 33 | 16.7 / 16.8 / 117 | 50.0 / 66.7 | 16.9 |

`stillBitmapSelectedHandles` holds the display's cadence in every run (p95 within 0.1 ms of the empty-frame p95) with script p95 1.4 to 3.7 ms. The 765 ms frame of the earlier agent-load run did not recur (maximum 20 to 33 ms). `stillBitmap` also holds the cadence but keeps single 83 to 117 ms frames. The other probes match the agent-load runs: `strokeOnly` holds the cadence; `fillOnly` (p95 19.6 to 33.4) and `outlineNoHandles` (p95 33.3 to 33.4) miss it; every variant that fills all outlines and all handles each frame has p95 50 to 117 ms.

### Earlier run: 2026-10-03, machine under load

The same benchmark ran three times while other agent sessions and builds kept the machine busy (CPU counter 15% to 65%). Kept for comparison; the idle runs above replace it.

| Measurement | 2026-10-03, loaded | 2026-10-04, idle |
|---|---|---|
| Empty `invoke`, main thread, p50 / p95 | 6.6 to 7.4 / 9.5 to 10.4 ms | 5.6 to 6.2 / 7.1 to 9.0 ms |
| Binary commit p95, 1 / 100 / 1,000 / 5,000 points | 9.4 to 11.8 / 10.4 to 11.7 / 13.0 to 14.1 / 22.5 to 25.0 ms | 7.8 to 8.5 / 8.1 to 8.7 / 10.8 to 11.6 / 17.0 to 17.3 ms |
| JSON commit p95, 5,000 points | 62.1 to 69.5 ms | 46.0 to 47.6 ms |
| 5,000-point patch stream: latency p95, late of 600 | 17.5 to 23.4 ms; 46 to 144 | 12.5 to 13.0 ms; 4 to 5 |
| Hit test in the WebView, grid, p95 | 0.3 to 0.8 ms | 0.3 to 0.4 ms |
| Hit test in the WebView, worst case | not run | p95 0.5 to 0.6 ms |
| Empty animation frames p50 / p95 | 17.8 to 17.9 / 18.3 to 18.5 ms | 16.6 to 16.7 / 16.7 to 16.8 ms |
| `cached` drag p50 / p95 | 35.7 to 36.1 / 53.6 to 56.8 ms | 33.3 / 33.4 ms |
| `rebuild` drag p50 / p95 | 35.7 to 53.9 / 53.8 to 73.3 ms | 33.3 / 50.0 ms |

## Decision

1. **Payload format: binary.** Raw request bodies and `tauri::ipc::Response` bytes, with typed arrays in the packed layout, are mandatory for outline data. JSON is acceptable for commands carrying under about 100 values.
2. **Hit-test budget: met.** The WASM kernel stays. In the WebView, idle, p95 is 0.3 to 0.4 ms on the grid glyph and 0.5 to 0.6 ms on the worst case (few large nested contours). No spatial index is needed for M0.
3. **Commit budget: re-state it.** The fixed cost of one `invoke` in WebView2 is about 6 ms at p50 and 7 to 9 ms at p95 on the idle machine, before any data moves, so "round trip p95 under 8 ms" measures WebView2, not the engine. Proposed budget: **engine reducer + patch encode under 8 ms p95, measured in Rust; the UI never blocks on a commit; the page-side round trip is recorded, not gated.** The mitigations stay: commits are asynchronous (the UI keeps its transient state until the patch arrives), and commits and patches carry only the touched points. The user accepted this budget on 2026-10-04 (ADR-0002, *Acceptance*).
4. **Drag frame budget: not met by `cached` as designed; met by a layered variant.** `Path2D` caching alone is not enough, because the GPU process re-fills every path on every frame. The canvas engine should (a) draw content that does not move during a gesture once per glyph revision into an offscreen bitmap and copy it each frame, (b) draw vectors per frame only for what moves, and (c) draw point handles only for the selection. Measured: p95 16.8 ms against an empty-frame p95 of 16.8 ms, script under 2.5 ms, GPU raster about 6 ms. Three idle runs confirmed it (p95 16.8 to 16.9 ms against an empty-frame p95 of 16.8 to 16.9 ms).
5. **ADR status.** ADR-0002 and ADR-0013 are **Accepted** (2026-10-04), each with an *Acceptance* section: ADR-0002 with the restated commit budget of point 3, ADR-0013 with the layering conditions of point 4.
6. **Kernel loading and the CSP.** The kernel is loaded from bytes inlined in the bundle (`kernel.ts`, `@typefaced/geometry-wasm/wasm?inline`), so no `fetch` runs; compiling them needs only `script-src 'wasm-unsafe-eval'`, which is set. The shipped `connect-src` (`ipc: http://ipc.localhost`) has no `'self'`, so wasm-bindgen's default `fetch` of the module would have been blocked. **Not verified:** a run under the bundled app's CSP; the `#/bench` route exists only in the dev frontend, so that check waits for the first real consumer of the kernel (M4).

## Reproduce

```bash
cargo test -p tf-geometry                    # unit and property tests
cargo bench -p tf-geometry --bench hit_test  # native hit test and translate timings
wasm-pack build crates/tf-wasm --target web --out-dir ../../packages/geometry-wasm/pkg -- --locked
pnpm --filter @typefaced/desktop build       # needed before any cargo command on the desktop crate
cargo test -p typefaced-desktop --features bench bench::
```

In-app benchmark (about 6 minutes per full run; about 2 minutes with `VITE_BENCH_ONLY=drag`):

```powershell
pnpm --filter @typefaced/desktop dev                 # leave running: serves #/bench on :1420
# drag-only instead: $env:VITE_BENCH_ONLY = "drag"; pnpm --filter @typefaced/desktop dev
cargo build --release -p typefaced-desktop --features bench   # 15 to 35 min the first time
$env:TYPEFACED_BENCH_EXIT = "1"                      # exit and write the results when done
& .\target\release\typefaced-desktop.exe | Out-Null  # Out-Null makes PowerShell wait for the window app
# results: target\bench-results.json; drawing probes under dragProbes; progress lines start with TYPEFACED_BENCH_LOG
```

GPU facts and traces: set `$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9222"` before starting the app, then call `SystemInfo.getInfo` and `Tracing.start` / `Tracing.end` on the browser target listed at `http://127.0.0.1:9222/json/version`. Add `--force_high_performance_gpu` to the same variable to run the WebView2 GPU process on the NVIDIA GPU. The scripts used for this report were throwaway and are not kept.

Or interactively: `pnpm --filter @typefaced/desktop tauri dev --features bench` (debug Rust, so the commit numbers will be worse). Never pass `bench` to `tauri build`.

## Follow-ups

- **M1: give the commit budget a number.** Time the Rust side of a commit (decode, reducer, patch encode) once real reducers exist, and gate it at 8 ms p95 (ADR-0002, *Acceptance*).
- Run the in-app benchmark on at least one other Windows machine with a 60 Hz display.
- M4 (`@typefaced/canvas`): cache `Path2D` per glyph and revision; cache still layers as bitmaps per revision and redraw vectors only for what moves during a gesture; draw handles for the selection only; add a frame-time regression test in the real WebView. Check the one-time cost of building the still bitmap at the start of a drag.
- The single 6.7 to 7.0 ms hit-test call per run: check whether it is the first call after the kernel loads.
