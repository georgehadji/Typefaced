# Spike 5: boolean engine for overlap removal

Decides [ADR-0008](../adr/0008-boolean-engine.md). Plan: [M0 Step 10](../../plans/typefaced-m0-foundations-and-spikes.md). Code: [`spikes/spike-boolean/`](../../spikes/spike-boolean/).

## Question

Static exports need overlap removal, and fontc 1.0 lacks it. The parametric engine and the "remove overlap" operation need a boolean union too (implementation plan §5.2, §5.3, §5.7). Which engine goes behind the `BooleanEngine` port:

- **skia-safe** (MIT bindings over Skia, BSD-3-Clause), PathOps;
- **linesweeper** (MIT OR Apache-2.0, early beta)?

Exit criteria for the winner: 0 crashes and an area error under 0.1% on every case, or the exceptions are documented; both engines run on at least 50 cases.

## Setup

| Item | Value |
|---|---|
| CPU | Intel Core i7-9750H, 6 cores / 12 threads, 2.60 GHz (laptop) |
| RAM | 32 GB |
| OS | Windows 11 Pro, build 26300 |
| Toolchain | rustc 1.98.1 (48a229cea 2026-09-01), `release` profile for the report run, `bench` profile for criterion |
| Crates | `skia-safe` 0.153.3 (default features, prebuilt Skia binaries), `linesweeper` 0.4.0, `kurbo` 0.13.1, `norad` 0.18.4 (feature `kurbo`), `criterion` 0.8.2 (default features off) |
| Load | Two other agents were building Rust and Node projects on the same machine for the whole spike. Timings are noisy (criterion's confidence intervals are ±15–25%); the ratios between engines are more reliable than the absolute numbers. Correctness results are deterministic. |

**Licenses.** `cargo deny --manifest-path spikes/spike-boolean/Cargo.toml --config deny.toml check licenses` passes: all 131 dependency packages are on the Appendix B allow-list (`skia-safe` and `skia-bindings` MIT, `linesweeper`, `kurbo`, `polycool`, `norad` MIT OR Apache-2.0). The Skia C++ code is not a Cargo package, so cargo-deny cannot see it. The prebuilt binary for the default feature set (the only set with prebuilts) statically links Skia (BSD-3-Clause), zlib (Zlib), expat (MIT), wuffs (Apache-2.0), libpng (libpng-2.0) and libjpeg-turbo (IJG AND BSD-3-Clause AND Zlib), per `skia-bindings` 0.153.3 `build_support/skia/config.rs`. libpng-2.0 and IJG are permissive but **not on the allow-list**; that matters only if Skia is ever adopted (see *Decision*). `cargo deny check advisories` on the spike also reports RUSTSEC-2026-0194 and RUSTSEC-2026-0195 in `quick-xml` 0.38.4, pulled in by `norad` 0.18.4 (see *Follow-ups*).

The spike is its own workspace (empty `[workspace]` table, own `Cargo.lock`). `cargo metadata` at the repository root lists only `tf-commands`, `typefaced-desktop` and `xtask`, and neither `skia-safe` nor `linesweeper` appears in the root dependency graph.

## Method

**Operation.** Remove overlaps under the non-zero rule, the fill rule of TrueType and CFF outlines:

- **skia-pathops:** `skia_safe::simplify` on a path with winding fill, then `skia_safe::as_winding`. Coordinates pass through f32, Skia's coordinate type.
- **skia-simplify-only** (diagnostic, not a candidate): `simplify` alone. Its output is meant for the even-odd rule.
- **skia-union-only** (diagnostic, not a candidate): `skia_safe::op` with `PathOp::Union` against an empty path. It shows that the direction failures below do not come from choosing `simplify` over `op`: the two give the same failure counts on all 581 cases.
- **linesweeper:** `linesweeper::binary_op(path, &BezPath::new(), FillRule::NonZero, BinaryOp::Union)`, the same call as fontc PR #2070.

Each run happens on its own thread with a 30 s timeout, so a panic or a hang is recorded instead of ending the run.

**Cases (581).** The case set is chosen by rules, not by hand, so best-case inputs cannot bias the verdict:

| Group | Cases | Rule |
|---|---:|---|
| source-sans | 548 | Every glyph of the three masters of `SourceSans3VF-Upright.designspace` whose own contours overlap (some region with \|winding\| ≥ 2) or mix directions (regions with both +1 and −1), above 0.5 unit². ExtraLight: 237 of 1,128 glyphs with contours; Upright (sparse master): 71 of 230; Black: 240 of 1,131. |
| inria-sans | 0 | Same rule over the six Inria Sans masters: none of their 274 glyphs with contours overlaps, so this family adds no cases. |
| fixtures | 4 | Every glyph with contours in `tests/fixtures/min.ufo` (`.notdef`, `A`, `O`, `acute`). None overlaps; they check that clean glyphs come back unchanged. |
| synthetic | 29 | Coincident edges (shared, partial, duplicate, reversed duplicate, duplicate circle, 50 collinear rectangles, edges 1e-4 apart); tangent circles (outside, inside, as a counter) and a square tangent to a circle; slivers (an overlap 0.005 wide, a gap 0.005 wide, a contour 0.007 thick, a 0.005 crescent between two circles); self-intersections (bow tie, cubic figure-eight, cubic self-loop, pentagram, a circle traced twice in one contour); 200 tiny overlapping circles; 200 tiny overlapping rotated squares; nested counters (alternating directions, same direction, two overlapping "O"s, an "Ø" slash across a counter); two circles at (30,000, 30,000); a zero-width spike; an empty path. |

Components are not decomposed (only each glyph's own contours), and open contours would be skipped (there are none in these sources).

**Reference.** Area error is measured against the input, without trusting either engine. Both paths are flattened with kurbo (tolerance 0.001 units) and integrated along 8,192 horizontal scanlines over the union bounding box (a step of at most 0.125 units for a 1,000-unit glyph). On each scanline the winding numbers of input and output are exact between edge crossings, so the error comes only from the flattening tolerance and the vertical step. This is a rasteriser with exact horizontal coverage. The plan suggested tiny-skia at 1,000 units per em; its 4 × 4 anti-aliasing would add up to 1/16 pixel of noise per boundary pixel, which is close to 0.1% on thin glyphs, so the scanline integrator was used instead. Its unit tests check a square (exact), a circle (within 10⁻⁵), a union of two squares, and the overlap and direction measures.

**Per case and engine:**

- panics, errors and timeouts;
- validity: open contours, non-finite coordinates, *overlap left* (output area with \|w\| ≥ 2), *mixed winding* (output has both w = +1 and w = −1 regions, the smaller of the two areas);
- *area error*: area of the symmetric difference between input and output, both read under non-zero, divided by the input's area (floored at 1 unit²); and the same with the output read under even-odd, which separates wrong geometry from wrong contour directions;
- tiny contours (under 1 unit²) and segment count, output against input;
- time: median of 5 runs in the report run; criterion means over whole groups.

Overlap left, mixed winding and area error count as failures above 0.1% of the input area.

Reproduce:

```bash
cargo xtask corpus fetch
cargo test  --release --manifest-path spikes/spike-boolean/Cargo.toml
cargo run   --release --manifest-path spikes/spike-boolean/Cargo.toml -- --report target/spike-boolean.md
cargo run   --release --manifest-path spikes/spike-boolean/Cargo.toml -- --report target/x.md --svg SourceSans3-ExtraLight/Q   # SVGs of one case
cargo bench --manifest-path spikes/spike-boolean/Cargo.toml
```

## Results

### Exit criteria

| Engine | Cases | Crashes (panics) | Errors | Timeouts | Cases failing | Worst area error |
|---|---:|---:|---:|---:|---:|---|
| **linesweeper** | 581 | 0 | 0 | 0 | **0** | 0.0017% (`tiny-circles-200`) |
| skia-pathops | 581 | 0 | 6 | 0 | 27 | 345.6% (`SourceSans3-ExtraLight/Q`) |
| skia-simplify-only (diagnostic) | 581 | 0 | 1 | 0 | 249 | 345.6% (`SourceSans3-ExtraLight/Q`) |
| skia-union-only (diagnostic) | 581 | 0 | 1 | 0 | 249 | 345.6% (`SourceSans3-ExtraLight/Q`) |

linesweeper meets the exit criteria: 0 crashes and an area error under 0.1% on every case. Its worst Source Sans error, 0.0013% (`Phi`), equals Skia's even-odd error on the same glyph, so it is the reference's own noise, not a difference in shape.

### Summary by engine and group (report run)

| Engine / group | Cases | Errors | Overlap left | Mixed winding | Area error > 0.1% | Even-odd area error > 0.1% | Cases with tiny contours (total) | Segments out/in, median (max) | Median µs | p95 µs | Max µs |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| linesweeper / source-sans | 548 | 0 | 0 | 0 | 0 | 0 | 0 (0) | 1.00 (2.00) | 1,321 | 2,744 | 9,856 |
| linesweeper / fixtures | 4 | 0 | 0 | 0 | 0 | 0 | 0 (0) | 1.00 (1.00) | 444 | 855 | 855 |
| linesweeper / synthetic | 29 | 0 | 0 | 0 | 0 | 0 | 4 (119) | 1.12 (2.75) | 498 | 72,524 | 222,928 |
| skia-pathops / source-sans | 548 | 5 | 14 | 6 | 14 | 0 | 0 (0) | 1.00 (2.00) | 400 | 1,014 | 3,138 |
| skia-pathops / fixtures | 4 | 0 | 0 | 0 | 0 | 0 | 0 (0) | 1.00 (1.00) | 242 | 317 | 317 |
| skia-pathops / synthetic | 29 | 1 | 0 | 2 | 0 | 0 | 1 (92) | 1.00 (3.00) | 202 | 7,464 | 40,131 |
| skia-simplify-only / source-sans | 548 | 0 | 243 | 2 | 243 | 0 | 0 (0) | 1.00 (1.94) | 356 | 972 | 2,999 |
| skia-simplify-only / fixtures | 4 | 0 | 0 | 0 | 0 | 0 | 0 (0) | 1.00 (1.00) | 158 | 282 | 282 |
| skia-simplify-only / synthetic | 29 | 1 | 2 | 1 | 2 | 0 | 1 (92) | 1.00 (3.00) | 185 | 7,340 | 26,772 |
| skia-union-only / source-sans | 548 | 0 | 243 | 2 | 243 | 0 | 0 (0) | 1.00 (1.94) | 363 | 987 | 1,933 |
| skia-union-only / fixtures | 4 | 0 | 0 | 0 | 0 | 0 | 0 (0) | 1.00 (1.00) | 206 | 354 | 354 |
| skia-union-only / synthetic | 29 | 1 | 2 | 1 | 2 | 0 | 1 (92) | 1.00 (3.00) | 180 | 7,952 | 23,976 |

The *Errors* column counts engine calls that returned no path. No engine panicked, and none produced open contours or non-finite coordinates. The median, p95 and max columns are single-run times from one report run, taken while other builds were running on the machine; the criterion table below is the timing to quote.

### Failures, case by case

**linesweeper:** none. Two quality defects that are not failures under the criteria:

- `coincident-curves` (a circle drawn twice) and `circle-traced-twice` each produce one extra debris contour of about 10⁻⁸ unit² next to the real outline. A font would need it removed.
- `tiny-circles-200` (25 tiny contours) and `tiny-squares-200` (92) contain small contours, but these are real holes between the shapes: Skia returns the same 92 for the squares.

**skia-pathops** (`simplify` + `as_winding`), 27 failing cases:

| Case | Failure |
|---|---|
| `tiny-circles-200` | `simplify` failed (returned no path) |
| ExtraLight `won`, `zcurl`, `dzcurl`; Upright `won`; Black `won` | `as_winding` failed (returned no path) |
| ExtraLight `Q` 345.6%, `Q.sc` 273.6%, `Q.sups` 238.6%, `koppa` 267.4%, `Ya` 146.0%, `Ya.sc` 119.4%, `ya` 111.4%, `command` 44.0%, `norights` 62.8% (also mixed winding 58.2%); Black `Q` 26.4%, `Q.sc` 18.0%, `Q.sups` 23.9%, `koppa` 15.0%, `Ya` 9.4% | counters wound like outer contours, so the non-zero rule fills them: overlap left and area error of the given size |
| ExtraLight `robot` 46.4%, `sharealike` 38.6%; Black `norights` 44.5%, `robot` 28.9%, `sharealike` 39.2%; `figure-eight-cubic` 50.0%; `nested-counters-alternating` 16.7% | mixed winding: filled regions with opposite directions (the fill is right, the directions are not) |

`op` with `Union` against an empty path gives the same failure counts as `simplify` alone (`skia-union-only` above), so choosing `op` instead of `simplify` does not change the result. Skia's geometry is right: read under even-odd, no Skia output is off by more than 0.0013%. Every non-error failure is a contour-direction failure. `simplify` returns non-overlapping contours for the even-odd rule: without `as_winding`, 245 of 548 Source Sans glyphs fail, almost all with counters that fill under non-zero. `as_winding` brings that down to 24: 14 glyphs still fill a counter (ExtraLight `norights` also mixes directions), 5 mix directions, and on 5 `as_winding` returns no path. For example, in ExtraLight `Q` the tail joins the bowl, and both the outer contour and the counter come back counter-clockwise (`--svg` shows it); linesweeper winds the counter clockwise.

### Time per glyph (criterion)

Criterion runs each engine over a whole group; only the 575 cases that both candidates complete are timed, so both engines run on the same set (10 samples, 20 s measurement).

| Group | Glyphs | skia-pathops | linesweeper | linesweeper / Skia |
|---|---:|---:|---:|---:|
| source-sans | 543 | 303.6 ms (0.56 ms per glyph) | 852.7 ms (1.57 ms per glyph) | 2.8× |
| fixtures | 4 | 0.53 ms (0.13 ms per glyph) | 1.60 ms (0.40 ms per glyph) | 3.0× |
| synthetic | 28 | 67.4 ms | 97.4 ms | 1.4× |

Criterion means of the final run (the 95% confidence interval for the Source Sans row is 287–320 ms for Skia and 829–879 ms for linesweeper). An earlier criterion run, with more build load on the machine, gave 427.9 ms and 1,187.8 ms (the same 2.8× ratio), so the absolute times move by about 40% with load and the ratio does not. The report run's medians agree: 0.40 ms (Skia) and 1.32 ms (linesweeper) per Source Sans glyph. linesweeper's slowest case is `tiny-circles-200` at 223 ms (Skia fails on it); its slowest Source Sans glyph takes 9.9 ms in the report run. If every glyph of a 2,500-glyph static master were as costly as these overlapping Source Sans glyphs, overlap removal would take about 3.9 s on one core with linesweeper and about 1.4 s with Skia. Both engines are pure functions of one glyph, so export can run glyphs in parallel.

### Build cost

Cold `cargo build --release --locked --no-default-features [--features <engine>]`, each in an empty target directory, so every dependency compiles from scratch; the Skia build includes downloading the prebuilt archive (5.2 MB for `x86_64-pc-windows-msvc`, default features). The baseline (no engine) still compiles `norad`, `kurbo` and the measurement code.

| Build | Cold build, run 1 | Cold build, run 2 | Cold build, run 3 (final code) | Release executable (run 3) | Delta vs. baseline |
|---|---:|---:|---:|---:|---:|
| baseline (no engine) | 319 s | 357 s | 304 s | 1,819,648 B | — |
| linesweeper | 308 s | 320 s | 371 s | 2,188,288 B | +368,640 B (+0.35 MiB) |
| skia-safe | 341 s | 471 s | 458 s | 5,132,800 B | +3,313,152 B (+3.16 MiB) |

Runs 1 and 2 used earlier versions of the spike's own code with the same dependencies; run 3 is the final code, re-measured at the end of the spike. The machine was shared with other build-heavy agents, so the build times of the same build vary by up to 60 s between runs, more than the difference between the baseline and linesweeper: linesweeper costs nothing measurable in build time. Skia was slower than the baseline in each run, by 22, 114 and 154 s (download, `skia-bindings` and `skia-safe`), so "Skia adds a minute or two" is the finding, not the exact seconds. Building Skia from source instead of using the prebuilts would cost far more and needs extra tools (Python, LLVM); that was not measured.

### Windows ARM64

Prebuilt Skia binaries for `aarch64-pc-windows-msvc` exist for skia-safe 0.153.3: the `rust-skia/skia-binaries` release `0.153.3` has 12 ARM64 Windows archives, including the default feature set (`…-aarch64-pc-windows-msvc-jpegd-jpege-pdf.tar.gz`, 4.4 MB). Not tested on ARM64 hardware. linesweeper is pure Rust and needs no prebuilt binaries for any target.

### fontc's linesweeper pull request

[googlefonts/fontc#2070](https://github.com/googlefonts/fontc/pull/2070) ("Overlap detection & removal primitives") is **open, not merged** (checked again on 2026-10-03 through the GitHub API: `state: open`, `merged: false`): opened 2026-07-28, last updated 2026-09-02, +296 lines in `fontdrasil`, no review decision. It adds `remove_overlaps` and `has_overlaps` on top of `linesweeper` 0.4.0, using exactly the call this spike measured (union with an empty path, non-zero), so the linesweeper results above also describe that pull request's behaviour. fontc is Apache-2.0, so reading the pull request was allowed. Static overlap removal in fontc itself (fontc issue #717) is still future work.

### Incidental finding: norad and the fixtures

`norad` 0.18.4 cannot load `tests/fixtures/min.ufo`: its GLIF parser rejects XML comments inside `<outline>` ("unexpected element"), and the fixtures use them. The spike strips comments before parsing the fixture glyphs (`norad::Glyph::parse_raw`). Comments are valid XML, so either the fixtures drop them or the M1 UFO reader must accept them; this also matters for Step 6 if fontc reads the fixtures through norad.

## Decision

**Accept linesweeper (0.4.0, pinned with `=`) as the engine behind the `BooleanEngine` port**, in an adapter crate outside `tf-geometry`.

Reasons:

1. **Correct on every case.** 0 crashes, 0 errors, 0 validity failures and a worst area error of 0.0017% on 581 cases, with contour directions right for the non-zero rule. Skia PathOps, used through its public API, failed 27 cases: 6 errors and 21 contour-direction failures, 14 of which fill counters (area errors up to 346%).
2. **No native build and no extra license work.** linesweeper is pure Rust under MIT OR Apache-2.0. Skia needs prebuilt binaries tied to a feature set that bundles libpng and libjpeg-turbo, whose licenses are not on the allow-list, plus f32 coordinates.
3. **The cost is speed.** linesweeper is about 3× slower (1.6 ms per glyph against 0.6 ms on Source Sans). That is acceptable for export and for an explicit "remove overlap" command, and glyphs can run in parallel. It is not fast enough for live previews of large glyph sets on every keystroke; nothing in M0–M2 needs that.

**Fallback: skia-safe PathOps `simplify` plus an in-house contour-direction pass** (orient each output contour by its nesting depth), not `as_winding`. The spike shows `simplify`'s geometry is right on 580 of 581 cases. It still failed on 200 tiny overlapping circles, and adopting it needs libpng-2.0 and IJG added to Appendix B (or a from-source Skia build without the PDF and JPEG code). The fallback is used only if linesweeper shows a defect in M1 that upstream does not fix.

Conditions for the adapter (M1):

- Drop output contours below a small area (the debris contours from coincident curves), with a test that real small counters survive.
- Keep this spike's 581 cases as a regression corpus: rerun on every linesweeper upgrade, because it is beta (0.x) software.

## Follow-ups

- **M1 (`tf-geometry` port, adapter crate).** Implement `BooleanEngine` on linesweeper `=0.4.0` with the debris filter above; property tests for area invariants (§5.2); port the scanline reference as the test oracle.
- **M1.** Measure the other operations the parametric engine needs (intersection, difference, `binary_op` between two real shapes); this spike measured only remove-overlap.
- **M1.** Decide how the UFO reader handles XML comments in `<outline>` (norad 0.18.4 rejects them) and whether the fixtures keep them. Check `quick-xml` RUSTSEC-2026-0194 and RUSTSEC-2026-0195 (via norad 0.18.4) before norad enters the root workspace; UFOs are untrusted input.
- **M1.** Watch fontc PR #2070. If it merges, fontc and Typefaced use the same engine, and compile-time overlap removal can move to fontc.
- **Step 12.** Add the spike commands above to `CLAUDE.md`, and record that ADR-0008 is Accepted.
