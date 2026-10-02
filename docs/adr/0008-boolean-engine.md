# ADR-0008: Boolean engine: skia-safe vs. linesweeper (from the spike)
Status: Accepted · Date: 2026-10-01

Source: [implementation plan](../implementation-plan.md) §5.2, §5.3, §5.7, §5.10, §5.13, §15 (R10), Appendix B; [research](../research.md) §6.1, §6.2; [M0 plan](../../plans/typefaced-m0-foundations-and-spikes.md) deviations 3 and 8, Steps 10 and 12; [Spike 5 report](../spikes/spike-5-boolean.md).

## Context

- Static exports need overlap removal, and fontc 1.0 lacks it (§5.13, Step 10). The parametric engine needs a boolean union too (§5.7), and `tf-ops` offers "remove overlap" (§5.3).
- Two Rust candidates (Step 10, research §6.2):
  - **skia-safe:** MIT bindings over Skia (BSD-3-Clause), using PathOps union/simplify. Builds are heavy; prebuilt Windows binaries exist.
  - **linesweeper:** MIT/Apache-2.0, early beta, published on crates.io.
- A linesweeper-based overlap-removal pull request to fontc was open in September 2026 (research §6.1). It is still open (fontc #2070, last updated 2026-09-02).
- `tf-geometry` compiles to WASM, so it must not depend on either engine (§5.2).

## Decision

- **Boolean operations go through the `BooleanEngine` port** (a Strategy; §5.2, §5.10). The engine adapters live outside `tf-geometry`, in an adapter crate, so the kernel stays WASM-friendly.
- **Chosen (Spike 5): linesweeper, pinned `=0.4.0`.** Remove-overlap is `linesweeper::binary_op(path, empty, FillRule::NonZero, BinaryOp::Union)`, the same call as fontc #2070. The adapter drops debris contours below a small area.
- **Fallback: skia-safe PathOps `simplify` plus an in-house contour-direction pass** (orient each contour by its nesting depth). Skia's own `as_winding` is not used. Adopting the fallback first needs libpng-2.0 and IJG added to Appendix B (they are bundled in the prebuilt Skia binaries), or a from-source Skia build without the PDF and JPEG code.

## Consequences

- On the spike's 581 cases (548 overlapping Source Sans master glyphs, 4 fixtures, 29 synthetic edge cases), linesweeper had 0 crashes, 0 errors, consistent contour directions and a worst area error of 0.0017%.
- Overlap removal costs about 1.6 ms per glyph (criterion mean on Source Sans), about 3× Skia; a 2,500-glyph master takes at most about 4 s on one core. Glyphs are independent, so export parallelises them.
- No native code, no prebuilt binaries and no license additions: linesweeper is pure Rust under MIT OR Apache-2.0, on every target. It adds 0.35 MiB to a release executable (Skia: 3.16 MiB).
- linesweeper is beta software (R10). The adapter keeps the spike's cases as a regression corpus, rerun on every upgrade, and the version is pinned.
- Two coincident-curve cases produced a debris contour of about 10⁻⁸ unit²; the adapter filters such contours.

## Alternatives considered

- **skia-safe PathOps through its public API (`simplify` then `as_winding`).** The geometry was right on every case it returned (even-odd area error at most 0.0013%), and it is about 3× faster. Rejected because, read under the non-zero rule fonts use, it failed 27 of 581 cases: `simplify` returned no path for 200 tiny overlapping circles, `as_winding` returned no path for 5 glyphs, 14 glyphs came back with filled counters (area errors up to 346%, e.g. Source Sans ExtraLight `Q`), and 7 more mixed contour directions. It also stores coordinates as f32 and needs prebuilt native binaries whose bundled libpng and libjpeg-turbo licenses are not on the allow-list.
- **Skia `simplify` alone, or `op` union with an empty path.** Their output is meant for the even-odd rule: 245 of 548 Source Sans glyphs fail under non-zero, with either call.
- **fontc's own overlap removal.** fontc #2070 is not merged; when it is, it uses the same engine.

## Validation

Validated by Step 10 — Spike 5 ([report](../spikes/spike-5-boolean.md)), which ran both engines on 581 cases chosen by rule (every overlapping glyph of the Source Sans 3 masters, every fixture glyph with contours, and synthetic coincident, tangent, sliver, self-intersecting, many-contour and nested cases), measuring crashes, errors, validity and area error against an engine-independent scanline reference. Exit criteria: both engines ran on at least 50 cases; the winner has 0 crashes and an area error under 0.1% on every case.

Enforcement and triggers to revisit:
- The M1 adapter crate runs the spike's case set as a regression test, with the scanline reference as the oracle, and `cargo-deny` checks linesweeper and its dependencies.
- Revisit if linesweeper fails a regression case that upstream does not fix, if remove-overlap time blocks an interactive feature, or when fontc #2070 merges.
