# ADR-0017: Persistent collections: `imbl` vs. an in-house chunked copy-on-write vector
Status: Proposed · Date: 2026-09-29

Source: [implementation plan](../implementation-plan.md) §5.1, §5.12, §10.4, Appendix B; [M0 plan](../../plans/typefaced-m0-foundations-and-spikes.md) deviation 3, Steps 11 and 12.

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
- **Spike 6 (Step 11) chooses** between the candidates and records the choice in this ADR.

## Consequences

- Either way, an edit costs O(chunk size), not O(glyph count), so 30,000-glyph CJK fonts stay fast (§5.1).
- With `imbl`, the MPL-2.0 file-level copyleft is respected by using the crate unmodified (Appendix B).
- With the in-house vector, Typefaced owns more code in `tf-model`, which targets at least 90% coverage (§5.1).

## Alternatives considered

- The two candidates above are the alternatives. The naive baseline is only the yardstick: its memory grows about 70% under the spike's load (Step 11).

## Validation

**Decided by Step 11 — Spike 6: persistent collections for snapshots.** Step 11 benchmarks each candidate at 1,000 and 30,000 glyphs of about 100 points each: snapshot clone, replacing one glyph, replacing 100 glyphs in one transaction, iterating over all glyphs, lookup by ID, and the memory held by 200 snapshots with one edit each (allocations counted with a counting `GlobalAlloc`).

Pass criteria (Step 11 exit criteria):
- The winner, at 30,000 glyphs:
  - replaces a single glyph in under 50 µs;
  - holds 200 single-edit snapshots in at most 10% more memory than the base document.
- These thresholds separate structural sharing from the naive baseline, whose memory grows about 70% under the same load.
- The decision is recorded in ADR-0017.

Step 11 sets this ADR to Accepted, naming the choice. The spike is off the critical path, and the user may defer it to M1 or M2 (deviation 3). A skipped spike leaves this ADR Proposed, and Step 12 lists it as a risk.
