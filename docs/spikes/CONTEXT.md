# docs/spikes/

Spike reports: the question, method, measurements and the verdict that decides an ADR.
The spike code lives in `spikes/`. Up: [docs/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `spike-1-fontc.md` | Spike 1 report for ADR-0006: runs fontc in-process through `crates/tf-compile/`, checks outputs with OTS, measures build time, binary size, compile speed and memory, and records the designspace and feature-file limits the UFO writer must respect |
| `spike-5-boolean.md` | Spike 5 report for ADR-0008: compares skia PathOps and linesweeper for overlap removal on overlapping corpus glyphs, fixtures and synthetic edge cases: correctness (area error, left-over overlap, contour directions), speed, build cost and licenses |
| `spike-6-collections.md` | Spike 6 report for ADR-0017: compares four glyph-table designs (naive `Arc<Vec>`, in-house chunked vector, `imbl::Vector`, `imbl::OrdMap`) on snapshot clone, single-glyph replace and memory of 200 undo snapshots, up to 65,535 glyphs |
