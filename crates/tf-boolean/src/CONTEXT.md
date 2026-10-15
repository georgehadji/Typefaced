# crates/tf-boolean/src/

Up: [tf-boolean/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `lib.rs` | `Linesweeper`, the `BooleanEngine` implementation: one `linesweeper::binary_op` call per operation under the non-zero rule (remove-overlap is a union with an empty path), non-finite input refused before linesweeper sees it, linesweeper errors mapped to `BooleanError`, and the debris filter (`DEBRIS_AREA`, with the measurements behind it) |
