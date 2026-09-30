# Spike 6: persistent collections for snapshots

Decides [ADR-0017](../adr/0017-persistent-collections.md). Plan: [M0 Step 11](../../plans/typefaced-m0-foundations-and-spikes.md). Code: [`spikes/spike-collections/`](../../spikes/spike-collections/).

## Question

The engine keeps every document state as an immutable snapshot, and undo history holds about 200 of them (implementation plan §5.1, §5.12). Which glyph table gives a cheap snapshot clone and a cheap single-glyph replace at 30,000 glyphs (CJK):

- `imbl` (MPL-2.0, used unmodified);
- an in-house chunked copy-on-write vector, `Arc<[Arc<Chunk>]>` with 64 glyphs per chunk;
- or, as the baseline, a naive `Arc<Vec<Arc<Glyph>>>`?

Pass criteria for the winner at 30,000 glyphs: replace one glyph in under 50 µs, and hold 200 single-edit snapshots in at most 10% more memory than the base document.

## Setup

| Item | Value |
|---|---|
| CPU | Intel Core i7-9750H, 6 cores / 12 threads, 2.59 GHz base (laptop, "High performance" power plan) |
| RAM | 32 GB (2 × 16 GB DDR4-3200) |
| OS | Windows 11 Pro, build 26200 (runs 1 and 2); build 26300 after an OS update (run 3) |
| Toolchain | rustc 1.98.1 (48a229cea 2026-09-01), cargo 1.98.1, `bench` profile (opt-level 3) |
| Crates | `imbl` 7.0.2 (MPL-2.0+), `criterion` 0.8.2 (Apache-2.0 OR MIT, default features off, `cargo_bench_support` only) |
| Load | Another agent session was running `cargo` builds on the same machine. Run 1 of the timing benchmarks ran while ~14 `rustc`/`cargo` processes were active; run 2 and the 65,535-glyph run ran after they had finished. The two runs agree within about 20%; the tables below use run 2. Run 3, after review, re-timed only the single-glyph edits at three positions on an otherwise idle machine; its numbers replace run 2's `replace_1` (see *Single edits by position*). The memory numbers are deterministic allocation counts and do not depend on load. |

**Absolute timings on this laptop are high.** Copying a 1,000-pointer `Vec<Arc<_>>` costs about 30–40 µs, roughly 30–40 ns per pointer. Each increment touches a separate 2.5 KiB glyph, so cache misses are a likely cause, but this was not profiled. Every candidate pays for reference counts in the same way, so the ranking holds; absolute numbers on other hardware are unknown until re-measured (see *Follow-ups*).

All dependencies (63 packages including transitive ones, from `cargo metadata`) are on the Appendix B allow-list: every package offers MIT or Apache-2.0 (Unlicense, Zlib and BSD-2-Clause appear only as `OR` alternatives), `unicode-ident` adds Unicode-3.0, and the `ciborium` crates are Apache-2.0 only. `imbl` and `imbl-sized-chunks` are MPL-2.0+ and are used unmodified.

## Method

- **Data.** Each glyph has a name and 4 contours × 25 points = 100 points. A `Point` is 24 bytes (`x`, `y`, kind, smooth), as in §5.1 without the optional point name. That is about 2.5 KiB per glyph, and the base document is 73.8 MiB at 30,000 glyphs.
- **Candidates**, behind one small trait (`GlyphTable`: `from_glyphs`, `get`, `replace(edits)`, `iter`):
  - `naive`: `Arc<Vec<Arc<Glyph>>>`. Every edit copies the whole vector.
  - `chunked`: `Arc<[Arc<[Arc<Glyph>]>]>`, 64 glyphs per chunk. `Arc::make_mut` copies the spine once per transaction and each touched chunk once.
  - `imbl-vector`: `imbl::Vector<Arc<Glyph>>` (RRB tree), indexed by `GlyphId`.
  - `imbl-ordmap`: `imbl::OrdMap<GlyphId, Arc<Glyph>>` (B+ tree). Added because the ADR describes a map from `GlyphId`, and stable IDs become sparse after deletions.
- **Correctness.** Unit tests run the same scenario on all four (a 100-glyph transaction, then single edits at chunk edges and on the last glyph of a 1,000-glyph table, whose last chunk is partial). They check that the final contents are identical, that `get` and `iter` agree, and that the base snapshot is unchanged.
- **Timing** (criterion, 100 samples, 3 s warm-up, 5 s measurement), at 1,000 and 30,000 glyphs, plus 65,535 (the OpenType maximum) in a separate run:
  - `clone`: take a snapshot.
  - `replace_1_first` / `replace_1_middle` / `replace_1_last`: one transaction replacing the first, middle or last glyph. Position matters for `imbl::Vector`, which keeps both ends of the tree in separate buffers. The exit criterion uses the slowest of the three.
  - `replace_100`: one transaction replacing 100 glyphs spread over the table (prime stride). Dropping the new snapshot is left out of the timing (`iter_with_large_drop`), because history evicts old snapshots off the edit path.
  - `iterate`: visit every glyph and read its contour count.
  - `lookup_1000`: 1,000 lookups by ID spread over the table.
- **Memory.** A counting `GlobalAlloc` (forwarding to `System`) tracks live requested bytes. For each candidate: build the base document, then create 200 snapshots, each the previous one with one more glyph (a different one each time) replaced by an edited copy, and keep all of them alive. "Growth" is the bytes held by the 200 snapshots divided by the base document's bytes. The edited glyphs themselves (about 0.5 MiB for 200) are part of any undo history, so the table also shows the growth without them, which is the cost of the data structure alone.

Reproduce:

```bash
cargo test  --manifest-path spikes/spike-collections/Cargo.toml
cargo bench --manifest-path spikes/spike-collections/Cargo.toml                 # both benches, all sizes
cargo bench --manifest-path spikes/spike-collections/Cargo.toml --bench memory  # memory table only
```

## Results

### Exit criteria at 30,000 glyphs

| Candidate | Replace 1 glyph (< 50 µs) | 200-snapshot growth (≤ 10%) | Passes |
|---|---:|---:|---|
| naive (baseline) | 1,179 µs | 62.7% | no |
| chunked | 23.7 µs | 2.7% | yes |
| imbl-vector | 6.6 µs | 1.6% | yes |
| imbl-ordmap | 3.6 µs | 1.0% | yes |

Replace times are the slowest of the three positions in run 3.

The naive baseline's memory grows 62.7% (the plan expected about 70%; the exact figure depends on the glyph size, since each snapshot copies 8 bytes per glyph against about 2.5 KiB per glyph of outline data).

### Timings (criterion median, run 2)

| Operation | Glyphs | naive | chunked | imbl-vector | imbl-ordmap |
|---|---:|---:|---:|---:|---:|
| clone | 1,000 | 64 ns | 67 ns | 391 ns | 85 ns |
| clone | 30,000 | 64 ns | 70 ns | 360 ns | 79 ns |
| clone | 65,535 | 58 ns | 64 ns | 363 ns | 73 ns |
| replace_100 | 1,000 | 54.7 µs | 98.6 µs | 97.3 µs | 150 µs |
| replace_100 | 30,000 | 1,564 µs | 463 µs | 517 µs | 304 µs |
| replace_100 | 65,535 | 3,516 µs | 472 µs | 520 µs | 480 µs |
| iterate | 1,000 | 4.9 µs | 5.5 µs | 40.0 µs | 66.7 µs |
| iterate | 30,000 | 291 µs | 282 µs | 1,284 µs | 2,008 µs |
| iterate | 65,535 | 551 µs | 480 µs | 3,900 µs | 5,578 µs |
| lookup_1000 | 1,000 | 8.8 µs | 14.5 µs | 126 µs | 255 µs |
| lookup_1000 | 30,000 | 16.1 µs | 24.4 µs | 287 µs | 609 µs |
| lookup_1000 | 65,535 | 12.5 µs | 25.4 µs | 332 µs | 595 µs |

Run 1 (under build load) gave the same ranking.

### Single edits by position (criterion median, run 3)

| Glyphs | Position | naive | chunked | imbl-vector | imbl-ordmap |
|---:|---|---:|---:|---:|---:|
| 1,000 | first | 29.9 µs | 3.4 µs | 2.3 µs | 2.1 µs |
| 1,000 | middle | 28.0 µs | 3.3 µs | 3.8 µs | 2.2 µs |
| 1,000 | last | 28.5 µs | 2.5 µs | 1.8 µs | 2.7 µs |
| 30,000 | first | 1,150 µs | 23.7 µs | 2.3 µs | 3.1 µs |
| 30,000 | middle | 1,141 µs | 22.6 µs | 6.6 µs | 3.1 µs |
| 30,000 | last | 1,179 µs | 21.3 µs | 1.9 µs | 3.6 µs |
| 65,535 | first | 2,807 µs | 50.4 µs | 2.3 µs | 3.4 µs |
| 65,535 | middle | 2,694 µs | **54.5 µs** | 7.3 µs | 3.3 µs |
| 65,535 | last | 2,707 µs | 52.9 µs | 2.4 µs | 3.8 µs |

Run 2 timed a single edit of glyph 0 only (the first position): 3.4 µs for `imbl::Vector` at 30,000 glyphs. That position is `imbl::Vector`'s best case, because it edits the front buffer without descending the tree. Review caught this, and run 3 replaced those numbers. Run 3's naive and chunked numbers are also lower than run 2's (1,179 vs. 1,807 µs for naive at 30,000 glyphs), which shows how much this machine's timings vary between runs.

### Memory held by 200 single-edit snapshots

| Candidate | Glyphs | Base (MiB) | Held by snapshots (MiB) | Growth vs. base | Growth without edited glyphs | Allocations |
|---|---:|---:|---:|---:|---:|---:|
| naive | 1,000 | 2.46 | 2.02 | 82.3% | 62.4% | 1,800 |
| chunked | 1,000 | 2.46 | 0.64 | 26.1% | 6.2% | 1,800 |
| imbl-vector | 1,000 | 2.46 | 0.85 | 34.3% | 14.4% | 1,928 |
| imbl-ordmap | 1,000 | 2.49 | 0.64 | 25.8% | 6.1% | 2,000 |
| naive | 30,000 | 73.76 | 46.27 | 62.7% | 62.1% | 1,800 |
| chunked | 30,000 | 73.77 | 2.03 | 2.7% | 2.1% | 1,800 |
| imbl-vector | 30,000 | 73.79 | 1.19 | 1.6% | 1.0% | 2,194 |
| imbl-ordmap | 30,000 | 74.67 | 0.74 | 1.0% | 0.3% | 2,400 |
| naive | 65,535 | 161.12 | 100.50 | 62.4% | 62.1% | 1,800 |
| chunked | 65,535 | 161.15 | 3.72 | 2.3% | 2.0% | 1,800 |
| imbl-vector | 65,535 | 161.19 | 1.20 | 0.7% | 0.4% | 2,197 |
| imbl-ordmap | 65,535 | 163.12 | 0.74 | 0.5% | 0.2% | 2,400 |

At 1,000 glyphs most of the growth is the 200 edited glyphs themselves (200 of 1,000 glyphs changed); the §10.4 budget (1,000 glyphs, 200 undo steps, under 500 MB) is met by every candidate, including the naive one.

The counter sees requested bytes, not allocator rounding or fragmentation. At 30,000 glyphs, 200 edits make 1,800 allocations for naive and chunked (9 per edit), 2,194 for `imbl::Vector` (about 11) and 2,400 for `imbl::OrdMap` (12). The `imbl` candidates' real overhead is therefore slightly higher than shown, but not by enough to approach 10%.

### What the numbers say

- **All three structural candidates meet both exit criteria at 30,000 glyphs.** The naive baseline fails both by a wide margin.
- **Writes.** `imbl::Vector` replace cost grows slowly in the middle of the tree (3.8, 6.6 and 7.3 µs at 1,000, 30,000 and 65,535 glyphs) and stays about 2 µs at either end. `imbl::OrdMap` stays at 2–4 µs. The chunked vector copies its spine (one pointer per 64 glyphs) on every transaction, so its cost grows linearly: about 3 µs at 1,000, 24 µs at 30,000 and 50–55 µs at 65,535 glyphs, just over the 50 µs budget on this machine.
- **Reads.** The chunked vector reads as fast as a plain `Vec`. `imbl::Vector` is about 4.5–8× slower to iterate and 9–13× slower per lookup (about 0.3 µs per lookup). `imbl::OrdMap` is 1.4–1.7× slower again to iterate and about 2× slower per lookup. In absolute terms these costs are small: a full pass over 30,000 glyphs takes 1.3 ms, and a canvas frame that resolves 300 glyphs spends about 0.1 ms on lookups.
- **Memory.** All structural candidates stay under 3%; `imbl` shares the most.

## Decision

**Accept `imbl` (7.x, unmodified) for persistent collections. The glyph table is an `imbl::Vector<Arc<Glyph>>` indexed by `GlyphId`.** The in-house chunked vector is not adopted.

Reasons:

Both `imbl` and the chunked vector pass the exit criteria at 30,000 glyphs, so the choice rests mainly on reasons 1 and 2; reasons 3 and 4 add margin.

1. **One dependency covers the other tables.** Kerning, groups and the name ↔ ID index also need persistent maps; `imbl::OrdMap` and `imbl::HashMap` cover them. The chunked vector covers only the dense glyph table, so Typefaced would still need a map library or more in-house code.
2. **Less code to own.** `imbl` is maintained and tested upstream. The MPL-2.0 file-level copyleft is respected by using the crate unmodified (Appendix B).
3. **Write cost that grows slowly.** `imbl::Vector` edits a glyph in at most 6.6 µs at 30,000 glyphs and 7.3 µs at 65,535, about 7× under the budget. The chunked vector's cost grows linearly and slightly exceeds the budget at the OpenType maximum on this machine (50–55 µs). That is a risk for the largest fonts, not a failure of the exit criteria, and the medians come from single runs on a noisy laptop. 100-glyph transactions cost about the same for both (463 vs. 517 µs at 30,000 glyphs).
4. **Reads are slower but cheap enough.** `imbl`'s read penalty (0.3 µs per lookup, 1.3 ms per full pass at 30,000 glyphs) is far below frame and compile budgets. If profiling in M1 or later shows a hot read path, a per-snapshot memoised cache (§5.12) is the first fix, not a different collection.

`imbl::OrdMap` wins on memory but costs about twice as much as `imbl::Vector` for reads. Glyph IDs stay dense: a deleted glyph leaves a tombstone slot (for example `Option<Arc<Glyph>>`), and IDs are compacted only on load or save, so the vector is the better fit for the glyph table.

## Follow-ups

- **M1 (`tf-model`).** Implement `GlyphTable` on `imbl::Vector`, with tombstones for deleted glyphs. Step 3.1 already allows MPL-2.0 in `deny.toml`, and `MPL-2.0` satisfies `imbl`'s `MPL-2.0+`, so no per-crate exception is needed; the "unmodified" condition is kept by never patching or vendoring the crate.
- **M1.** Benchmark `imbl::OrdMap` and `imbl::HashMap` for kerning (tens of thousands of pairs) and the name index before choosing between them; this spike measured glyph-sized values only.
- **M1.** Re-run this spike's single-edit and `iterate` numbers on CI hardware, with several runs to show the spread, and keep a regression benchmark for the 50 µs single-edit budget in `tf-model` that edits glyphs in the middle of the table, not only at the ends.
- **Step 12.** Add the spike commands above to `CLAUDE.md`, and record that ADR-0017 is Accepted.
