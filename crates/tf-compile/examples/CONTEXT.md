# crates/tf-compile/examples/

Small programs that time the compile for Spikes 1 and 2 (ADR-0006, ADR-0007). Each
prints the elapsed milliseconds. Up: [tf-compile/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `compile.rs` | Compiles a `.ufo` or `.designspace` to a TTF file with `compile_to_ttf`, or a `.ufo` to an OTF file with `compile_to_otf` when the output ends in `.otf`. Run: `cargo run --release -p tf-compile --example compile -- <input> <output.ttf\|output.otf>` |
| `baseline.rs` | Same shape as `compile.rs` but only copies a file, to measure the cost of the process itself without fontc |
