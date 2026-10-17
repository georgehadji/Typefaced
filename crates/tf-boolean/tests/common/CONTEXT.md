# crates/tf-boolean/tests/common/

Test support, ported from Spike 5 (`spikes/spike-boolean/`). Used by both test targets and,
through `#[path]`, by `benches/ops.rs`. Up: [tests/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `mod.rs` | Declares the two modules |
| `cases.rs` | The case set: 29 synthetic edge cases, the fixture glyphs of `tests/fixtures/min.ufo` (XML comments stripped, because norad 0.18.4 rejects them), and the overlapping glyphs of the corpus masters, selected by rule; shape helpers for the property tests |
| `oracle.rs` | The scanline oracle: areas, overlap, mixed winding and symmetric difference under the non-zero rule over a whole case (`compare`), or inside one thin feature at a finer tolerance (`coverage`); structural checks of an output (`structure`) |
