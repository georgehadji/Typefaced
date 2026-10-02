# spikes/spike-boolean/

Spike 5 (ADR-0008): which boolean engine removes overlaps behind the `BooleanEngine` port,
skia PathOps or linesweeper. It runs both on overlapping corpus glyphs, the test fixtures
and synthetic edge cases. Report: `docs/spikes/spike-5-boolean.md`. Up: [spikes/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `Cargo.toml` | Own workspace root; depends on `kurbo`, `norad`, and the optional engines `skia-safe` and `linesweeper` (one Cargo feature each, so a build can measure one engine alone); `criterion` for benchmarks only; declares the `engines` bench |
| `Cargo.lock` | Locked dependencies of this spike |

| Subfolder | Context |
|---|---|
| `src/` | [CONTEXT.md](src/CONTEXT.md): engines, case set, reference measurements and the report runner |
| `benches/` | [CONTEXT.md](benches/CONTEXT.md): timing benchmark |
