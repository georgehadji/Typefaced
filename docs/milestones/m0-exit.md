# M0 exit review: foundations and spikes

Date: 2026-10-14 · Plan: [M0 blueprint](../../plans/typefaced-m0-foundations-and-spikes.md), Step 12 · Exit criteria: [implementation plan](../implementation-plan.md) §14, row M0.

**Verdict.** Each of the three M0 exit criteria has evidence. Every spike ran, and all 19 ADRs are Accepted. No spike was skipped. Some budgets were restated or met only with conditions: the commit budget, the drag frame budget and the fontc compile margin (see *Numbers against budgets*). M1 can start once the user has reviewed this report.

All evidence below comes from existing artifacts: merged PRs, CI runs, spike reports and ADRs. No benchmark and no live AI test was run again for this review.

## Exit criteria and evidence

### 1. CI green on Windows

- The latest `main` run is [CI run 37890768807](https://github.com/georgehadji/Typefaced/actions/runs/37890768807), on `405ddb2` (merge of #14), 2026-10-09. Result: **success**.
  - Job `windows` (windows-latest): every step passed. Steps: install, WASM build, frontend build, fmt, clippy (also with the `bench` feature), `cargo test --workspace`, bindings diff, `pnpm lint && pnpm typecheck && pnpm test:ci`, `cargo xtask check-deps`, the API-key check, the clean-tree check and, as a push to `main`, "Build the app (debug, no installer)".
  - Job `gates` (ubuntu-latest): `cargo deny check`, `cargo xtask licenses-npm`, `cargo xtask context check` and `cargo xtask coverage` passed.
- The two runs before it on `main` also succeeded: [37308624264](https://github.com/georgehadji/Typefaced/actions/runs/37308624264) (`6c2b255`, #13) and [37233355129](https://github.com/georgehadji/Typefaced/actions/runs/37233355129) (`7a32f2e`, #12).
- The pipeline came from Step 3.1 ([#4](https://github.com/georgehadji/Typefaced/pull/4), xtask gates) and Step 3.2 ([#5](https://github.com/georgehadji/Typefaced/pull/5), CI workflows).

### 2. A typed IPC round-trip

- **Step 2, [#2](https://github.com/georgehadji/Typefaced/pull/2)** (`cef2fe6`) built the walking skeleton. `AppInfo` in `crates/tf-commands` becomes the Tauri command `app_info`, tauri-specta (`=2.0.0-rc.25`) generates `commands.appInfo()` into `packages/bindings/src/index.ts`, and React renders the result.
  - Tests: `app_info_serializes_to_the_ipc_json_shape` and `app_info_round_trips_through_json` in `tf-commands`, and `export_bindings` in the desktop crate.
  - Every CI run checks that the committed bindings match the generated file (step "Bindings are up to date").
  - #2 also checked the debug app through the DevTools protocol: the window shows `Typefaced v0.1.0`, read from `tauri.conf.json` through Rust and the typed binding.
- **A second typed path, Steps 9.1/9.2 ([#13](https://github.com/georgehadji/Typefaced/pull/13), [#14](https://github.com/georgehadji/Typefaced/pull/14)).** The `ai_*` commands are typed tauri-specta commands too (`aiSetKey`, `aiHasKey`, `aiDeleteKey`, `aiFetch` with a typed `Channel<ProxyEvent>`, `aiAbort`). The live smoke test of 2026-10-08 used them end to end ([spike 4](../spikes/spike-4-ai-egress.md), *Live smoke test*).
- **IPC performance, Spike 3 ([#12](https://github.com/georgehadji/Typefaced/pull/12)).** The benchmark commands are raw Tauri commands outside tauri-specta. They give the latency numbers below; they are not evidence of typing.

### 3. Every spike answered, with its ADR

| Spike | Step · PR | ADR | Verdict | Report |
|---|---|---|---|---|
| 1 fontc in-process | 6 · [#9](https://github.com/georgehadji/Typefaced/pull/9) (`992aef6`) | [ADR-0006](../adr/0006-compiler-fontc-in-process.md) Accepted 2026-10-03 | fontc 1.0.0 runs in-process; all 9 outputs pass OTS; conditions listed below | [spike-1-fontc.md](../spikes/spike-1-fontc.md) |
| 2 CFF writer + OTF transplant | 7 · [#11](https://github.com/georgehadji/Typefaced/pull/11) (`42a9841`) | [ADR-0007](../adr/0007-cff-otf-via-ttf-transplant.md) Accepted 2026-10-03 | `tf-cff` built in-house; 0 point differences on the fixture and 6 Inria Sans styles; OTS, `ttx`, fontTools and Windows Font Viewer pass | [spike-2-cff.md](../spikes/spike-2-cff.md) |
| 3 IPC latency + WASM kernel | 8 · [#12](https://github.com/georgehadji/Typefaced/pull/12) (`7a32f2e`) | [ADR-0002](../adr/0002-rust-engine-owns-document-state.md), [ADR-0013](../adr/0013-ui-stack-react-zustand-canvas2d-wasm.md) Accepted 2026-10-04 | Binary payloads; hit test met; commit budget restated; layered drawing needed for 60 fps | [spike-3-ipc.md](../spikes/spike-3-ipc.md) |
| 4a AI egress host (Rust) | 9.1 · [#13](https://github.com/georgehadji/Typefaced/pull/13) (`6c2b255`) | [ADR-0009](../adr/0009-ai-orchestration-typescript-sdk-rust-egress.md) | Policy, streaming, abort and vault tests pass | [spike-4-ai-egress.md](../spikes/spike-4-ai-egress.md) |
| 4b AI client + tool runner (TS) | 9.2 · [#14](https://github.com/georgehadji/Typefaced/pull/14) (`405ddb2`) | [ADR-0009](../adr/0009-ai-orchestration-typescript-sdk-rust-egress.md) Accepted 2026-10-09; [ADR-0011](../adr/0011-model-configuration.md) amended | SDK over the Rust proxy to OpenRouter; live smoke test and debug-build CSP check pass | [spike-4-ai-egress.md](../spikes/spike-4-ai-egress.md) |
| 5 Boolean engine | 10 · [#10](https://github.com/georgehadji/Typefaced/pull/10) (`559fbad`) | [ADR-0008](../adr/0008-boolean-engine.md) Accepted 2026-10-03 | linesweeper: 0 failures on 581 cases; Skia PathOps failed 27 | [spike-5-boolean.md](../spikes/spike-5-boolean.md) |
| 6 Persistent collections | 11 · [#3](https://github.com/georgehadji/Typefaced/pull/3) (`daad381`) | [ADR-0017](../adr/0017-persistent-collections.md) Accepted 2026-09-30 | `imbl::Vector<Arc<Glyph>>` | [spike-6-collections.md](../spikes/spike-6-collections.md) |

The other ADRs (0001, 0003–0005, 0010, 0012, 0014–0016, 0018, 0019) were Accepted as written in Step 4 ([#1](https://github.com/georgehadji/Typefaced/pull/1), `10c5081`). The check from the Step 12 brief prints nothing:

```text
$ grep -l '^Status: Proposed' docs/adr/0*.md
$
```

**Other M0 work.**
- Step 1: the bootstrap commit `6504860` on `main`.
- Step 5: test fixtures and the pinned corpus ([#6](https://github.com/georgehadji/Typefaced/pull/6), `17f429f`).
- Two PRs outside the step list:
  - [#7](https://github.com/georgehadji/Typefaced/pull/7) (`680e219`) replaced the yanked `yoke-derive` with 0.8.4;
  - [#8](https://github.com/georgehadji/Typefaced/pull/8) (`b7b76f5`) added the `CONTEXT.md` maps, the context gate and `AGENTS.md`.

## Numbers against budgets

All numbers were measured on one laptop: i7-9750H, 32 GB, Windows 11, WebView2. The conditions are given for each row, because machine load changed the results by up to about 40%.

| Budget (source) | Measured | Result |
|---|---|---|
| Hit test, all contours of a 5,000-point glyph, < 1 ms (§7.3, §10.4) | WebView, idle: grid p95 0.3–0.4 ms; worst case (nested 2 × 2,500 and 5 × 1,000) p95 0.5–0.6 ms, p99 0.6–0.8 ms | **Met**, worst case included (spike 3) |
| WASM kernel size, < 350 KB gzipped (§5.2) | 36.5 KiB raw, 16.3 KiB gzipped | **Met** (spike 3) |
| Commit round trip p95 < 8 ms (§10.4, as written) | An empty `invoke` alone: p50 5.6–6.2 ms, p95 7.1–9.0 ms. Binary commit p95: 1 point 7.8–8.5 ms, 100 points 8.1–8.7 ms, 1,000 points 10.8–11.6 ms, 5,000 points 17.0–17.3 ms (JSON at 5,000 points: 46.0–47.6 ms) | **Not met as written. Restated and accepted 2026-10-04** (ADR-0002): engine reducer + patch encode < 8 ms p95, measured in Rust; the UI never blocks on a commit; the page-side round trip is recorded, not gated. **The Rust-side gate has no number yet.** The spike's reducer only swaps an `Arc`; M1 measures it |
| Drag of a 5,000-point glyph at 60 fps, frame p95 < 16 ms (§7.3, §10.4) | Read by the spike as "within 2 ms of the empty-frame cadence (16.7 ms at 60 Hz)". `cached` as first designed: p95 33.4 ms idle (30 fps). Layered variant (`stillBitmapSelectedHandles`): p95 16.8–16.9 ms against an empty-frame p95 of 16.8–16.9 ms in three idle runs | **Met only with the layered drawing conditions** of ADR-0013. 16.8 ms is not below 16 ms literally; it equals the display's own cadence |
| Export static TTF, 1,000 glyphs, < 2 s (§10.4) | Spike 1, fat LTO, saturated machine: Inria Sans (591 glyphs) median 993 ms over 30 runs, about 1.7 s per 1,000 glyphs. Spike 2, LTO off, about 11% load: `compile_to_ttf` median 420 ms for 591 glyphs | **Met with a small margin** (about 15% in spike 1). Re-time on an idle machine before M2 sets the CI budget |
| Export static OTF, 1,000 glyphs, < 3 s (§10.4) | Spike 2, LTO off, about 11% load: `compile_to_otf` median 779 ms for 591 glyphs, about 1.3 s per 1,000 glyphs | **Met** |
| Overlap removal: 0 crashes, area error < 0.1% on every case (Step 10) | linesweeper: 581 cases, 0 crashes, 0 errors, 0 failures, worst 0.0017%. Speed 1.57 ms per glyph against Skia's 0.56 ms (criterion, Source Sans) | **Met** by linesweeper. Limit: the area metric cannot prove that thin slivers survive |
| Single-glyph replace < 50 µs; 200 snapshots ≤ 10% memory, at 30,000 glyphs (Step 11) | `imbl::Vector`: 6.6 µs (slowest position, run 3); growth 1.6%. Chunked vector 23.7 µs and 2.7%; naive 1,179 µs and 62.7% | **Met** |
| Memory, 1,000 glyphs with 200 undo steps, < 500 MB (§10.4) | Synthetic glyphs (100 points each): base 2.46 MiB + 0.85 MiB for 200 snapshots with `imbl::Vector` | **Met** for the data structure only; the real model is measured in M1 |
| fontc binary size and build time (Step 6, recorded, no budget) | +14.4 MB in the release binary; cold release build 42 min 54 s, of which 25 min was the fat-LTO link | Recorded |
| AI egress pass criteria (ADR-0009) | `tf-ai-host` 52 tests, 97.97% line coverage; `@typefaced/ai` 33 tests; no key-shaped string in the bundle; live smoke test (3 requests, all 200) and debug-build CSP check pass | **Met** |

## Decisions that changed during M0

| What changed | Where it is recorded |
|---|---|
| **Commit budget restated.** It is now engine reducer + patch encode < 8 ms p95, measured in Rust. The UI never waits, and the page-side round trip is recorded, not gated. Binary payloads are mandatory for outline data; commits and patches carry only the touched points | ADR-0002 *Acceptance* (2026-10-04); plan §10.4, §7.2 |
| **Layered canvas drawing.** Still content goes into a bitmap once per revision, vectors are drawn only for what moves, and handles only for the selection. `Path2D` caching alone gives 30 fps | ADR-0013 *Acceptance* (2026-10-04); plan §7.3 |
| **No spatial index (R-tree) for M0.** Per-contour bounding-box culling meets the hit-test budget | Spike 3 *Decision* 2; plan §5.2, §7.3 |
| **AI provider: OpenRouter** (user decision 2026-10-05). The client uses an OpenRouter key and the Anthropic-compatible `POST /api/v1/messages`; the SDK is kept with `baseURL` `https://openrouter.ai/api`. App-side model routes per task use up to 3 OpenRouter fallbacks. The egress policy allows only that one endpoint | ADR-0009 amendment, ADR-0011 amendment; plan §1, §6.2, §6.4, §15 |
| **Key entry crosses the webview once** (deviation 9). Three risks are accepted (see *New risks*) | ADR-0009 |
| **`tf-cff` is built, not reused.** write-fonts 0.52.0 has no CFF 1 writer, and allsorts 0.17.0 cannot write a new font. write-fonts' `FontBuilder` is reused for the font assembly | ADR-0007; spike 2; plan §5.8 |
| **fontc input rules.** Designspace 4 features only, one shared `features.fea` for all masters, no XML comments inside glyphs. fontc panics are caught (`catch_unwind`) | ADR-0006; spike 1; plan §5.13 |
| **Boolean engine: linesweeper `=0.4.0`**, in an adapter crate. The fallback is Skia `simplify` plus an in-house direction pass | ADR-0008; plan §5.2, §15 R10 |
| **Glyph table: `imbl::Vector<Arc<Glyph>>`**, with tombstones for deleted glyphs | ADR-0017; plan §5.1 |
| **Licence allow-list.** `Unlicense` was added (user, 2026-10-05; `fast-sha256` via `@anthropic-ai/sdk`). `Apache-2.0 WITH LLVM-exception` was added in Step 3.1 (`target-lexicon`) | `deny.toml`; plan Appendix B (both now listed) |
| **AI model.** The plan's "`claude-opus-5` for every playbook" became the per-task routes above (`agent` → `anthropic/claude-opus-5.5`, `chat` → `anthropic/claude-sonnet-5.5`) | ADR-0011; plan §6.4; M0 plan change log |

The M0 blueprint also lists deliberate deviations from the implementation plan (its *Deviations* section): crates are created on demand, Biome is used, spike crates live outside the root workspace, and coverage gates are set per layer.

## Skipped spikes and new risks

**Skipped spikes: none.** No ADR is still Proposed (see the `grep` above).

**Accepted risks (ADR-0009):**
1. Script in the webview can replace or delete the stored key, because `ai_set_key` and `ai_delete_key` are callable from the main window. This closes in M3 with a native OS credential prompt.
2. The system proxy is honoured, so a TLS-inspecting corporate proxy can see the key and the traffic.
3. Prompts, including font data, go to whichever upstream provider OpenRouter selects. The control is the user's OpenRouter account privacy settings.

**New risks found by the spikes:**
- **quick-xml advisories.** RUSTSEC-2026-0194 and RUSTSEC-2026-0195 reach us through `norad` 0.18.4 and are ignored in `deny.toml`. Impact: denial of service when the user opens a crafted UFO or designspace. Remove the ignores when norad moves to quick-xml ≥ 0.41.
- **fontc panics.** A designspace with a discrete axis panics while fontc sets up the work, and the panic reaches the caller. An instance in user coordinates panics inside a fontc job, which fontc reports as an error. `catch_unwind` in `compile_to_ttf` turns the first kind into an error only while the release profile keeps `panic = "unwind"`.
- **norad rejects XML comments inside `<glyph>`.** It cannot open third-party UFOs that contain them, and the fixtures had to be changed.
- **linesweeper is 0.x beta software.** Keep the 581-case regression corpus and rerun it on every upgrade. The area metric does not prove that thin features survive.
- **Ajv under the CSP.** Ajv compiles validators with `new Function`, which the shipped CSP blocks. Production code (M3) must use precompiled (standalone) validators.
- **Kernel load under the bundled CSP has not been run.** It waits for the first real consumer of the kernel (M4).
- **One machine.** Every benchmark ran on one laptop, often under load from other builds. Spike 1's fontc margin under 2 s is about 15%.
- **OpenRouter is a second party.** Its availability, pricing (it reserves credit for the full `max_tokens`) and data handling now matter.

## Carry-over items for M1 and later

**M1 (headless engine):**
- Give the commit gate a number: a Rust benchmark of decode, reducer and patch encode, gated at 8 ms p95 (ADR-0002).
- `tf-model`: `GlyphTable` on `imbl::Vector` with tombstones; a regression benchmark for the 50 µs single edit at the middle of the table; benchmark `imbl::OrdMap` and `imbl::HashMap` for kerning; re-run the spike 6 numbers on CI hardware.
- `BooleanEngine` on linesweeper `=0.4.0` in an adapter crate: a debris filter for tiny contours, the 581 cases as a regression corpus, the scanline oracle plus a local check for thin features; measure intersection and difference.
- UFO reader: decide how to handle XML comments in `<outline>`. Check the quick-xml advisories before norad enters the engine's read path.
- Watch fontc PR #2070 (linesweeper-based overlap removal) and norad releases.

**M2 (build pipeline):**
- `FontCompiler` port: keep panic capture, and decide on a worker process if the profile moves to `panic = "abort"`.
- Re-time the static TTF on an idle machine.
- Consider `lto = "thin"` for local release builds.
- Add a variable family with one shared feature file to the corpus.
- `tf-cff`: operator merging, Top DICT FontInfo, `vmtx`/`VORG` support, `.N` duplicate names (or `SourceOutlines` built from the engine's own names), static instances from a designspace, and the rendering comparison against fontmake.
- CI: run `ttx` and OTS on `target/min.otf` (ADR-0007 asks for them on every change).
- Report the fontc panics and the `NonIdenticalFea` limitation upstream.

**M3 (AI platform):**
- Native credential prompt (closes accepted risk 1).
- Ajv standalone validators.
- Record the served model in the ledger.
- Re-issue a turn when tool JSON cannot be parsed (capped).
- Refusals as an HTTP-like 403 head.
- The user's decision on `provider.data_collection` / `zdr`.

**M4 (editor):**
- Implement the layered canvas and a frame-time regression test in the real WebView.
- Measure the one-time cost of building the still bitmap.
- Run the kernel under the bundled CSP.
- Run the in-app benchmark on a second Windows machine.

**Agent instructions.** A proposed `AGENTS.md` update adds the commands the spikes introduced, and the WASM build that must run before the frontend build. It waits for the user's approval and is not part of this change.

## Next blueprint (proposal)

Proposal: once the user has reviewed this report and the Step 12 PR is merged, run `/ecc:blueprint typefaced "M1 headless engine"` in a fresh session. Its scope is §14's M1 row:
- `tf-model`;
- `tf-engine` (actor, reducers, history, sandboxes, journal);
- about 30 commands in `tf-commands`;
- `tf-io` (UFO/designspace and `.typefaced`);
- `tf-cli` (`info`, `convert`).

Its first steps should take the M1 carry-over items above:
- the Rust-side commit benchmark;
- `imbl::Vector` with tombstones;
- the linesweeper adapter and its regression corpus;
- the UFO reader's handling of XML comments and of the quick-xml advisories.

Its exit criteria are those of §14: a byte-stable UFO round trip on the corpus, green undo and sandbox property tests, and ≥ 90% coverage. Nothing has been started.
