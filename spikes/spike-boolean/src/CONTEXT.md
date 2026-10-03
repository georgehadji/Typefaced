# spikes/spike-boolean/src/

Up: [spike-boolean/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `lib.rs` | Library root: declares the `cases`, `engines` and `measure` modules, shared by the report runner and the benchmark |
| `engines.rs` | The candidate engines behind one non-zero remove-overlap function: Skia PathOps (`Simplify` then `AsWinding`), Skia `Simplify` alone and Skia `Op` union as diagnostics, and linesweeper union. Also `run_guarded`, which runs an engine on its own thread with a timeout so a panic or hang is recorded |
| `cases.rs` | The case set: corpus glyphs whose own contours overlap or mix directions (Source Sans 3 and Inria Sans masters from the downloaded corpus), every contour-bearing glyph of `tests/fixtures/min.ufo`, and hand-built synthetic edge cases |
| `measure.rs` | Engine-independent reference measurements: area, symmetric difference, left-over overlap and mixed winding by exact-coverage scanline integration, plus structural checks on an output path (open contours, non-finite values, tiny contours, segment count) |
| `main.rs` | The report runner: runs every engine on every case, judges each result against the area tolerance and writes a Markdown report; `--svg <case>` dumps the input and each engine's output as SVG |

Case names and winner conclusions are in the report, not here.
