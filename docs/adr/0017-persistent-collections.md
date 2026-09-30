# ADR-0017: Persistent collections: `imbl` vs. an in-house chunked copy-on-write vector
Status: Accepted · Date: 2026-09-30

Source: [implementation plan](../implementation-plan.md) §5.1, §5.12, §10.4, Appendix B; [M0 plan](../../plans/typefaced-m0-foundations-and-spikes.md) deviation 3, Steps 11 and 12; [Spike 6 report](../spikes/spike-6-collections.md).

## Context

- The engine keeps every document state as an immutable snapshot, and undo history holds about 200 of them ([ADR-0003](0003-persistent-state-snapshot-undo-actor-rcu.md); §5.1, §5.12, Step 11).
- Glyph tables need a cheap clone and a cheap single-glyph replace, even at 30,000 glyphs (CJK fonts) (Step 11).
- Memory budget: a 1,000-glyph project with 200 undo steps stays under 500 MB (§10.4).

## Decision

- **The glyph table is a persistent, structurally shared collection** (§5.1): a map from `GlyphId` to `Arc<Glyph>`, plus a name index and the glyph order.
- **Candidates** (§5.1, Step 11):
  - `imbl` (MPL-2.0; allowed if unmodified, Appendix B);
  - an in-house chunked copy-on-write vector, `Arc<[Arc<Chunk>]>` with 64 glyphs per chunk.
- **The baseline** for comparison is a naive `Arc<Vec<Arc<Glyph>>>` (Step 11).
- **Chosen (Spike 6): `imbl`, used unmodified.** The glyph table is an `imbl::Vector<Arc<Glyph>>` indexed by `GlyphId`; a deleted glyph leaves a tombstone slot, and IDs are compacted only on load or save. Kerning, groups and the name index use `imbl` maps (which map is benchmarked in M1).
- **Not adopted:** the in-house chunked vector.

## Consequences

- A single-glyph edit costs O(log n): at most 3.8, 6.6 and 7.3 µs at 1,000, 30,000 and 65,535 glyphs on the spike machine (in the middle of the tree; about 2 µs at either end), against a 50 µs budget. 200 single-edit snapshots add 1.6% to a 30,000-glyph document (Spike 6).
- Reads are slower than a plain vector: about 0.3 µs per lookup by ID and 1.3 ms for a full pass over 30,000 glyphs, 4.5–13× the chunked vector's cost. Hot read paths use memoised per-snapshot caches (§5.12).
- One dependency also provides the persistent maps that kerning, groups and the name index need.
- The MPL-2.0 file-level copyleft is respected by using the crate unmodified (Appendix B); `imbl` is never patched or vendored with changes.

## Alternatives considered

- **In-house chunked copy-on-write vector** (`Arc<[Arc<Chunk>]>`, 64 glyphs per chunk). It passes both criteria at 30,000 glyphs (at most 23.7 µs per replace, 2.7% memory growth) and reads as fast as a plain vector. Rejected mainly because it covers only the dense glyph table, not the maps, and would be more code to own. Also, every edit copies the whole spine, so the cost grows linearly and slightly exceeds the budget at the OpenType maximum of 65,535 glyphs (50–55 µs on the spike machine).
- **`imbl::OrdMap` as the glyph table.** Least memory (1.0% growth), but reads cost about twice as much as `imbl::Vector`; dense IDs with tombstones make the map unnecessary.
- **Naive `Arc<Vec<Arc<Glyph>>>`** (the yardstick): 1.2 ms per replace and 62.7% memory growth at 30,000 glyphs; fails both criteria.

## Validation

Validated by Step 11 — Spike 6 ([report](../spikes/spike-6-collections.md)), which benchmarked all candidates at 1,000, 30,000 and 65,535 glyphs of 100 points. At 30,000 glyphs `imbl::Vector` replaces one glyph in at most 6.6 µs, the slowest of the first, middle and last glyph (criterion: under 50 µs), and holds 200 single-edit snapshots in 1.6% more memory than the base document (criterion: at most 10%). The naive baseline grows 62.7%.

Enforcement and triggers to revisit:
- `tf-model` keeps a regression benchmark for the 50 µs single-edit budget at 30,000 glyphs, editing glyphs in the middle of the table (M1).
- `cargo-deny` checks `imbl` and `imbl-sized-chunks` once `tf-model` depends on them (M1): MPL-2.0 is on the allow-list since Step 3.1, and code review keeps the crates unmodified.
- Revisit if profiling shows `imbl` reads on a hot path that caching cannot fix, or if `imbl` stops being maintained.
