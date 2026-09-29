# ADR-0008: Boolean engine: skia-safe vs. linesweeper (from the spike)
Status: Proposed · Date: 2026-09-29

Source: [implementation plan](../implementation-plan.md) §5.2, §5.3, §5.7, §5.10, §5.13, §15 (R10); [research](../research.md) §6.1, §6.2; [M0 plan](../../plans/typefaced-m0-foundations-and-spikes.md) deviations 3 and 8, Steps 10 and 12.

## Context

- Static exports need overlap removal, and fontc 1.0 lacks it (§5.13, Step 10). The parametric engine needs a boolean union too (§5.7), and `tf-ops` offers "remove overlap" (§5.3).
- Two Rust candidates (Step 10, research §6.2):
  - **skia-safe:** MIT bindings over Skia (BSD-3-Clause), using PathOps union/simplify. Builds are heavy; prebuilt Windows binaries exist.
  - **linesweeper:** MIT/Apache-2.0, early beta, published on crates.io.
- A linesweeper-based overlap-removal pull request to fontc was open in September 2026 (research §6.1).
- `tf-geometry` compiles to WASM, so it must not depend on either engine (§5.2).

## Decision

- **Boolean operations go through the `BooleanEngine` port** (a Strategy; §5.2, §5.10). The engine adapters live outside `tf-geometry`, in an adapter crate, so the kernel stays WASM-friendly.
- **Spike 5 (Step 10) picks the adapter:** skia-safe PathOps or linesweeper, chosen by robustness on a corpus of cases (§5.10, R10). The spike records the winner and the fallback in this ADR.

## Consequences

- The engine can be swapped without touching domain code.
- skia-safe adds build weight; linesweeper adds beta-quality risk (R10). The spike measures both.
- The spike is its own Cargo workspace, so its heavy dependencies never enter product CI (Step 10, deviation 8).

## Alternatives considered

- The decision is between the two candidates above. If fontc's linesweeper-based pull request has merged, Step 10 evaluates it as well.

## Validation

**Decided by Step 10 — Spike 5: boolean engine for overlap removal.** Step 10 runs both engines' union/remove-overlap on at least 50 cases: overlapping glyphs from the corpus variable family, the test fixtures, and synthetic cases (coincident edges, tangent circles, slivers thinner than 0.01 units, a self-intersecting figure-eight, 200 tiny overlapping contours, nested counters). It records crashes and errors, output validity, area error against a reference, time per glyph, cold build time, binary size, and whether prebuilt skia-safe binaries exist for Windows ARM64.

Pass criteria (Step 10 exit criteria):
- Both engines have run on at least 50 cases.
- The winner shows 0 crashes and an area error under 0.1% on every case, or the exceptions are documented.
- The decision is recorded in ADR-0008.

Step 10 sets this ADR to Accepted, naming the choice and the fallback. The spike is off the critical path, and the user may defer it to M1 or M2 (deviation 3). A skipped spike leaves this ADR Proposed, and Step 12 lists it as a risk.
