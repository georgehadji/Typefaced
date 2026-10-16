# crates/tf-boolean/src/

Up: [tf-boolean/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `lib.rs` | `Linesweeper`, the `BooleanEngine` implementation: one `linesweeper::binary_op` call per operation under the non-zero rule (remove-overlap is a union with an empty path), non-finite input and coordinates beyond `MAX_COORDINATE` refused before linesweeper sees them, linesweeper errors mapped to `BooleanError`, and the debris filter (`DEBRIS_AREA`, with the measurements behind it) |
