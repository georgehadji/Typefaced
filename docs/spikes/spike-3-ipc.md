# Spike 3: IPC latency and WASM geometry kernel

Tests [ADR-0002](../adr/0002-rust-engine-owns-document-state.md) and [ADR-0013](../adr/0013-ui-stack-react-zustand-canvas2d-wasm.md). Plan: [M0 Step 8](../../plans/typefaced-m0-foundations-and-spikes.md). Code: [`crates/tf-geometry`](../../crates/tf-geometry/), [`crates/tf-wasm`](../../crates/tf-wasm/), [`packages/geometry-wasm`](../../packages/geometry-wasm/), [`apps/desktop/src-tauri/src/bench.rs`](../../apps/desktop/src-tauri/src/bench.rs), [`apps/desktop/src/bench/`](../../apps/desktop/src/bench/).

**Verdict in one line.** The hit test is fast on a typical layout (p95 about 0.3 ms in the WebView) but its **worst case, few large nested contours, is not shown to meet the 1 ms budget**: under Node it sits at the edge (p95 0.53 to 1.00 ms) and the WebView run of that case is still to be done. The binary payload format wins clearly above 100 points. The commit round-trip budget (p95 under 8 ms) and the drag frame budget (p95 under 16 ms) are **not met on this machine, and no payload format or drawing change measured here fixes that**. Both ADRs stay Proposed and carry an amendment. The step's exit criteria are not met, so the step is not marked done.

## Question

Do the budgets of implementation plan §10.4 and §7.3 hold on this Windows machine, with the Rust engine owning the document and the UI dragging locally on a WASM geometry kernel?

- Commit round-trip (IPC + reducer + patch): p95 under 8 ms.
- Drag of a 5,000-point glyph at 60 fps: frame time p95 under 16 ms.
- Hit-test across all contours of the 5,000-point glyph: under 1 ms.
- Which payload format should the engine use, JSON or binary?

## Setup

| Item | Value |
|---|---|
| Machine | HP Pavilion Gaming Laptop 17-cd0xxx, Intel Core i7-9750H (6 cores / 12 threads, 2.6 GHz base), 32 GB RAM, Intel UHD Graphics 630 + NVIDIA GTX 1050, 1920×1080 at 60 Hz, "High performance" power plan |
| OS and WebView | Windows 11 Pro build 26300; WebView2 with Chromium 154 (user agent `Chrome/154.0.0.0 Edg/154.0.0.0`) |
| Toolchain | rustc 1.98.1, cargo 1.98.1, Node 24.14.0, wasm-pack 0.15.0 |
| Crates | tauri 2.12.0, wasm-bindgen 0.2.129, kurbo 0.13.1, proptest 1 (tests only), serde_json 1 (`bench` feature only) |
| In-app build | `cargo build --release -p typefaced-desktop --features bench` (release profile: opt-level 3, LTO), run directly as `target\release\typefaced-desktop.exe` with `TYPEFACED_BENCH_EXIT=1`. Without the `custom-protocol` feature the binary loads the `devUrl`, so the frontend came from `pnpm --filter @typefaced/desktop dev` (Vite dev server, unminified JS). The `#/bench` route exists only in that dev frontend. The WASM package was the release build from `wasm-pack` (36.5 KiB raw, 16.3 KiB gzipped; the §5.4 budget is 350 KB gzipped) |
| Machine load | Other agent sessions share this laptop. The CPU load counter read 15% after run 1, 20% after run 2 and 65% after run 3 (run 3 has the worst tails in the drag and patch-stream rows). The native `cargo bench` runs were made at 8% to 24%. An earlier run of the same code, while builds were running on the machine, gave the same picture with slower tails |
| Runs | The in-app benchmark ran three times in a row (runs 1, 2, 3). Tables show the range over the three runs unless one number is given |

## Method

- **Glyph.** A synthetic glyph of 50 contours × 100 points = 5,000 points (`makeGlyph` in `outline.ts`, mirrored by `benches/hit_test.rs`). Each contour is a wavy ring: one line point, then cubic segments (off, off, curve). Points are about 3 font units apart. The packed layout is the §7.2 one: `Float64Array` coordinates, `Uint8Array` flags, `Uint32Array` contour ends.
- **Worst-case glyph.** The grid glyph is the best case for a hit test: its 50 contours have disjoint bounding boxes, so the per-contour box check skips about 49 of 50 contours. The worst case is a glyph of few large concentric contours (the counters of an "8", a target) with the pointer inside the box of every contour, so nothing is skipped. `makeNestedGlyph` builds two of them, both of 5,000 points: 2 contours × 2,500 points and 5 contours × 1,000 points. The outermost ring has about 3 units between points like the grid glyph, the radii shrink in equal steps, and the 1,000 (in-app) or 2,000 (native) positions are uniform inside the innermost ring's bounding box. The native bench and the in-app runner build the same glyphs.
- **Hit test.** `hit_test` finds the nearest point within the radius, otherwise the nearest segment (kurbo `nearest`), across all contours. Two radii: 8 units (mostly point hits) and 1 unit (finds segments). 1,000 deterministic positions over the glyph's bounding box in-app, 2,000 natively.
- **Empty round trip.** 1,000 `invoke` calls of a command that returns an empty body, once handled on the main thread and once on the async runtime.
- **Commit round trip.** 1,000 calls (after 20 warm-up calls) per payload format and size. An edit of 1, 100, 1,000 or 5,000 points goes to Rust, which swaps in a new `Arc` and returns a patch of the same size. JSON: edit as a JSON array, patch as a JSON string. Binary: edit as a raw body of little-endian `f64`, patch as raw bytes (16-byte header, then coordinates). The timer covers building the payload, the call, and turning the answer into a `Float64Array`. The plan asks for the 5,000-point size; the smaller sizes were added to separate the fixed cost of a call from the cost of the payload.
- **Patch stream.** Rust sends 600 binary patches at 16.67 ms over a `Channel` (10 s), once with 1-point patches and once with 5,000-point patches. Latency is measured from the send time stamped by Rust to the arrival in the page (same clock). "Late" means delivered more than one frame (16.67 ms) after sending.
- **Drag.** A synthetic 10 s pointer drag; each `requestAnimationFrame` moves the pointer on a circle, calls the WASM `hitTest` and `translatePoints` (1,000 of 5,000 points selected), then draws on a 760×760 Canvas2D. Three variants: `compute` (no drawing), `rebuild` (new `Path2D` objects for the whole glyph every frame, outline fill and stroke plus 5,000 handle squares) and `cached` (`Path2D` objects built once; the dragged contours are drawn with a canvas transform; this is the design in ADR-0013). Frame time is the `requestAnimationFrame` delta; work time is script time per frame. An empty `requestAnimationFrame` loop (3 s) gives the cadence of this window before any work.
- **Headless kernel timings.** `cargo bench -p tf-geometry` (native, release) and the same WASM package under Node (V8), with the same glyph and positions. These exclude the WebView.

## Results

### Budgets against the measurements

| Criterion | Budget | Measured | Result |
|---|---|---|---|
| Hit-test, all contours of the 5,000-point glyph | under 1 ms | **Grid layout (best case, contours skipped by box):** WASM in the WebView p95 0.4 / 0.4 / 0.8 ms (radius 8), 0.3 / 0.3 / 0.4 ms (radius 1); WASM under Node p95 0.30 to 0.38 ms; native p95 0.07 to 0.23 ms. **Nested layouts (worst case, no contour skipped):** WASM under Node p95 0.53 to 1.00 ms (p99 up to 1.56 ms); native p95 0.49 to 0.63 ms (max up to 2.3 ms); **WebView: not yet measured** (the in-app benchmark now runs it) | **Met for the grid layout. Not shown for the worst case**: it is at the budget under Node, and the WebView is not expected to be faster, so it may exceed 1 ms |
| Commit round trip, p95, better format (binary) | under 8 ms | 5,000 points: 24.4 / 22.5 / 25.0 ms. Even 1 point: 11.8 / 9.4 / 10.2 ms. An empty command: 9.5 to 11.5 ms | **Not met** |
| Drag frame time, p95 | under 16 ms | Empty animation frames already give p95 18.3 to 18.5 ms here. `cached`: 53.6 to 56.8 ms. `rebuild`: 53.8 to 73.3 ms | **Not met** (see below) |
| Numbers and chosen payload format recorded | | This report; binary | Done |

### Empty round trip (ms)

| Handler | p50 | p95 | p99 |
|---|---|---|---|
| Main thread | 6.6 to 7.4 | 9.5 to 10.4 | 11.8 to 13.4 |
| Async runtime | 7.2 to 8.3 | 10.4 to 11.5 | 12.0 to 14.4 |

An `invoke` that does nothing already costs about 7 ms at the median and 10 ms at p95. That is the floor for every command on this machine. The thread hop of the async runtime adds under 1 ms.

### Commit round trip (ms, range over three runs)

| Points | Payload (JSON / binary) | JSON p50 | JSON p95 | Binary p50 | Binary p95 | Binary p99 |
|---|---|---|---|---|---|---|
| 1 | 28 B / 16 B | 7.2 to 8.0 | 9.4 to 11.0 | 7.2 to 8.2 | 9.4 to 11.8 | 12.4 to 14.0 |
| 100 | 3.6 KB / 1.6 KB | 8.4 to 9.1 | 10.8 to 13.0 | 7.3 to 8.2 | 10.4 to 11.7 | 12.8 to 14.5 |
| 1,000 | 35.7 KB / 16 KB | 15.6 to 15.9 | 22.5 to 23.1 | 9.5 to 10.1 | 13.0 to 14.1 | 15.3 to 16.6 |
| 5,000 | 177 KB / 80 KB | 47.4 to 49.5 | 62.1 to 69.5 | 17.1 to 18.1 | 22.5 to 25.0 | 26.3 to 29.1 |

- Up to 100 points JSON and binary are equal within the noise. At 1,000 points binary is about 1.7× faster at p95 and at 5,000 points about 2.8× faster.
- The cost above the floor scales with the payload: at 5,000 points binary adds about 10 ms to the median, JSON about 40 ms. The reducer itself (swap an `Arc`) does not show: 1-point commits are as fast as the empty command.
- The Rust side was not timed separately; these are page-side round trips.

### Patch stream (600 patches at 60 Hz)

| Patch | Delivery latency p50 | p95 | Late (over 16.7 ms), of 600 | Inter-arrival p95 |
|---|---|---|---|---|
| 1 point | 1.4 to 1.5 ms | 1.9 to 2.1 ms | 2, 4, 4 | 17.3 ms |
| 5,000 points | 12.6 to 13.8 ms | 17.5 to 23.4 ms | 85, 46, 144 | 18.9 to 20.5 ms |

Small patches stream well (about 1.5 ms one way). Whole-glyph patches at 60 Hz arrive in 13 ms at the median and miss a frame 8% to 24% of the time. Patches must scale with the change, not with the glyph (ADR-0002 already says so).

### Hit test and translate, headless

| Case | p50 | p95 | p99 |
|---|---|---|---|
| `hit_test`, radius 8, native (3 runs) | 0.055 ms | 0.067 to 0.111 ms | 0.089 to 0.165 ms |
| `hit_test`, radius 1, native (3 runs) | 0.061 ms | 0.064 to 0.112 ms | 0.085 to 0.145 ms |
| `translate_points`, 1,000 of 5,000 selected, native | 0.23 to 0.25 ms | 0.32 to 0.41 ms | 0.36 to 0.53 ms |
| WASM `hitTest` under Node, radius 8 | 0.18 ms | 0.31 to 0.51 ms | 0.40 to 0.89 ms |
| WASM `translatePoints` under Node | 0.22 ms | 0.33 to 0.34 ms | 0.45 to 0.70 ms |
| `hit_test`, grid 50 × 100 (best case), radius 8 / 1, native (3 runs) | 0.078 to 0.084 ms | 0.099 to 0.172 ms | 0.145 to 0.347 ms |
| `hit_test`, nested 2 × 2,500 (worst case), radius 8 / 1, native (3 runs) | 0.32 to 0.48 ms | 0.51 to 0.62 ms | 0.63 to 0.99 ms |
| `hit_test`, nested 5 × 1,000 (worst case), radius 8 / 1, native (3 runs) | 0.31 to 0.43 ms | 0.49 to 0.63 ms | 0.58 to 0.88 ms |
| WASM `hitTest` under Node, grid, radius 8 / 1 (3 runs) | 0.19 to 0.22 ms | 0.30 to 0.38 ms | 0.36 to 0.54 ms |
| WASM `hitTest` under Node, nested 2 × 2,500, radius 8 / 1 (3 runs) | 0.47 to 0.48 ms | 0.79 to 1.00 ms | 0.89 to 1.56 ms |
| WASM `hitTest` under Node, nested 5 × 1,000, radius 8 / 1 (3 runs) | 0.47 to 0.48 ms | 0.53 to 0.94 ms | 0.73 to 1.23 ms |

The rows with ranges above give the lowest and highest of the six numbers (two radii × three runs). The worst-case rows were measured while other agent sessions kept the machine busy (the CPU counter read 32% to 100% around these runs), so their tails are pessimistic; the single slowest call in a run reached 1.0 to 2.3 ms natively. The WASM-under-Node numbers come from a throwaway script that imports `makeGlyph`, `makeNestedGlyph` and `innerBox` from `outline.ts` and times 2,000 `hitTest` calls after 200 warm-up calls; it is not kept in the repository. The in-app numbers for these layouts are in `hitTestWorstCase` of the results JSON and were not yet measured when this was written.

The native `translate_points` time is dominated by allocating and filling the 80 KB copy, which is noisy on this machine (the same copy ranged from 15 µs to 300 µs in a separate probe). Under WASM both calls also copy the arrays into and out of WASM memory.

Two changes made the hit test fast enough to have margin. The first version built a `Vec` of all 1,700 segments per call and measured 0.42 to 0.53 ms at p50 (p95 about 0.9 ms, close to the budget). Visiting segments without allocating, and skipping contours whose bounding box is out of reach, brought it to 0.055 ms at p50. The property tests that compare `hit_test` against a brute-force search still pass.

That speed depends on the layout. Culling by contour box only helps when the pointer is outside most boxes. On the nested glyphs every box contains the pointer, so every segment is visited and the hit test costs three to five times more (native p50 0.32 to 0.48 ms against 0.08 ms; under Node p50 0.47 ms against 0.19 ms). A reviewer's separate native measurements agree (50 × 100 grid p95 0.134 ms; 2 × 2,500 nested p95 0.368 ms; 5 × 1,000 nested p95 0.32 to 0.40 ms). Real glyphs are likely between the two: most letters have one to three contours, nested or side by side.

### Drag (760×760 canvas, 10 s each)

| Variant | Frame time p50 | p95 | Script work p50 | Script work p95 |
|---|---|---|---|---|
| Empty animation frames (3 s) | 17.8 to 17.9 ms | 18.3 to 18.5 ms | | |
| `compute` (hit test + translate, no drawing) | 17.9 to 18.0 ms | 18.4 to 18.6 ms | 0.4 to 0.5 ms | 0.6 to 0.8 ms |
| `rebuild` (new paths every frame) | 35.7, 35.8, 53.9 ms | 53.8 to 73.3 ms | 5.1, 5.1, 8.5 ms | 8.5, 8.7, 10.8 ms |
| `cached` (paths built once) | 35.7, 35.7, 36.1 ms | 53.6 to 56.8 ms | 0.5 to 0.6 ms | 0.9 to 1.0 ms |

What this shows and what it does not:

- **The kernel is cheap.** With drawing off, the frame cadence equals the empty-frame cadence and script time is under 1 ms. The WASM kernel is not the problem.
- **Caching the paths is needed.** Rebuilding the geometry each frame costs 5 to 9 ms of script time at the median (10.8 ms p95 in the worst run), over half a frame. Cached paths cost about 0.5 ms. This confirms the `Path2D` caching in ADR-0013.
- **The frame budget as written cannot be tested on this window.** An empty `requestAnimationFrame` loop already runs at a 17.8 ms median (about 56 Hz), although the display reports 60 Hz. A frame budget of 16 ms cannot be met by any code here. The cause was not investigated.
- **Drawing halves the frame rate for a reason not found.** With `cached` paths and under 1.5 ms of script time, frames arrive every 35.7 ms (every second vsync), so the page runs at about 28 fps. The cost is therefore outside the script, in rasterising or compositing (the laptop has a hybrid Intel/NVIDIA GPU; whether Canvas2D ran on the GPU was not checked). Ad-hoc probes of the same drawing code through the WebView2 remote-debugging port, run after the benchmark and not kept in the repository, gave 18 ms medians in some page layouts and 36 ms in others, in no order I could explain, and occasional stalls of 50 to 400 ms. Because this swings between runs of identical code, it is not a stable measurement of the architecture.
- **Handle squares are expensive.** Exploratory runs, again not kept, drew the outline alone at the 18 ms cadence and the outline plus all 5,000 handle squares with stalls (p95 about 90 ms). The cached design draws handles only for the selected contours.

## Decision

1. **Payload format: binary.** Raw request bodies and `tauri::ipc::Response` bytes, with typed arrays in the packed layout, are mandatory for outline data. JSON is acceptable for commands carrying under about 100 values.
2. **Hit-test budget: met on a typical layout, not shown for the worst case.** The WASM kernel stays. Its cost depends on the layout: culling by contour bounding box makes the grid glyph cheap (WebView p95 0.3 to 0.8 ms) and does nothing for few large nested contours, where it is about 0.5 ms at p50 and up to 1.0 ms at p95 under Node. The in-app WebView number for the worst case is the first thing to read from the next run. If it is over 1 ms the options are a spatial index per glyph revision, or testing segments by their own control box before building them; neither is needed until a measurement asks for it.
3. **Commit round trip: budget not met, and it cannot be met by changing the payload here.** The fixed cost of one `invoke` is about 7 ms at p50 and 10 ms at p95 on this machine, before any data moves. Mitigations:
   - A commit is asynchronous. The UI keeps its transient state until the patch arrives and never blocks on the round trip.
   - Commits and patches carry only the touched points, not whole outlines. At 100 points binary p95 is 10.4 to 11.7 ms, within about 1.5 ms of the floor.
   - The 8 ms budget needs to be re-stated to name what it measures, and re-measured on other hardware (see *Follow-ups*). This is a proposal: the ADR-0002 amendment says the budget failed and suggests the restatement, but its text is unchanged. The first two mitigations are recorded in the ADR-0002 amendment.
4. **Drag frame budget: not demonstrated.** What is shown: kernel cost is negligible, and `Path2D` caching is required. What is not shown: that a drag reaches the display's cadence in this WebView. ADR-0013 records this as open.
5. **ADR status.** ADR-0002 and ADR-0013 stay **Proposed**. They get an amendment section with the findings above. They become Accepted when the manual run below shows the commit round trip and drag frame time within budget on hardware where an empty frame loop runs at 16.7 ms, or when the budgets are formally re-stated. The plan's fallback ("move drag state further into the UI") already describes the architecture, so no structural change is proposed.
6. **Kernel loading and the CSP.** The first bench run loaded the module with wasm-bindgen's default, `fetch('tf_wasm_bg.wasm')`, against the Vite dev server, where the shipped CSP does not apply. In the shipped build `fetch` is governed by `connect-src`, which is `ipc: http://ipc.localhost` and has no `'self'`, so the same call would have been blocked. The Tauri CSP guide says Tauri adds nonces and hashes to the script and style directives at compile time; it does not say Tauri adds anything to `connect-src`, and the guide and the configuration reference say nothing on devUrl or `devCsp`. Rather than add a `connect-src` source on an assumption, kernel loading no longer needs the network: `apps/desktop/src/bench/kernel.ts` imports the module as a base64 data URL (`@typefaced/geometry-wasm/wasm?inline`, with `assetsInclude: ["**/*.wasm"]` in `vite.config.ts`), decodes it and passes the bytes to `init({ module_or_path })`. Compiling those bytes needs only `script-src 'wasm-unsafe-eval'`, which is set, and the CSP is unchanged. A unit test loads the kernel this way with `fetch` spied on and runs a hit test. **Not verified:** a run inside the WebView under the bundled app's CSP. The `#/bench` route exists only in the dev frontend, so that check waits for the first real consumer of the kernel (M4). The inlined module adds about 49 KB of base64 to the JS chunk that imports it.

## Reproduce

```bash
cargo test -p tf-geometry                    # unit and property tests
cargo bench -p tf-geometry --bench hit_test  # native hit test and translate timings
wasm-pack build crates/tf-wasm --target web --out-dir ../../packages/geometry-wasm/pkg -- --locked
pnpm --filter @typefaced/desktop build       # needed before any cargo command on the desktop crate
cargo test -p typefaced-desktop --features bench bench::
```

In-app benchmark, as run for this report (about 4 minutes per run):

```powershell
pnpm --filter @typefaced/desktop dev                 # leave running: serves #/bench on :1420
cargo build --release -p typefaced-desktop --features bench   # 15 to 35 min the first time
$env:TYPEFACED_BENCH_EXIT = "1"                      # exit and write the results when done
& .\target\release\typefaced-desktop.exe | Out-Null  # Out-Null makes PowerShell wait for the window app
# results: target\bench-results.json (hit-test worst case: hitTestWorstCase); progress lines start with TYPEFACED_BENCH_LOG
```

Or interactively: `pnpm --filter @typefaced/desktop tauri dev --features bench` (debug Rust, so the commit numbers will be worse). Never pass `bench` to `tauri build`.

## Follow-ups

- **Manual run on idle hardware (needed to accept the ADRs).** Close other programs, plug in the charger, keep the app window visible and in front, and run the in-app benchmark three times as above, on this machine and on at least one other Windows machine with a 60 Hz display. Check three things: (1) the empty round trip (`ping.main.p95`) and the 1-point commit p95 against the 8 ms budget; (2) `frameBaseline.p95` is about 16.7 ms, and `drag.cached.frameTime.p95` is within 2 ms of it; (3) the 5,000-point binary commit p95. If an idle machine still shows a 7 ms empty round trip, the commit budget must be re-stated (for example as "engine reducer + patch under 8 ms, UI never waits").
- **Read the hit-test worst case first.** In `targetench-results.json`, `hitTestWorstCase.nested2x2500` and `.nested5x1000` (each with `radius8` and `radius1`) give `perCall.p95` in the WebView. Compare with `hitTest` and `hitTestSegments` (the grid layout). Over 1 ms at p95 means the hit test needs a spatial index per glyph revision or a cheaper segment pre-check, and ADR-0013 gets that follow-up.
- Find out why drawing halves the frame rate: check whether Canvas2D is GPU-accelerated in this WebView2 (`chrome://gpu` equivalent through the remote-debugging port), and try the same page on the NVIDIA GPU.
- Time the Rust side of a commit (decode, reducer, encode) separately, to show how much of the floor is the WebView2 custom-protocol call.
- M4 (`@typefaced/canvas`): cache `Path2D` per glyph and revision, draw handles for the selection only, and add a frame-time regression test in the real WebView.
