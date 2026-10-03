# crates/tf-compile/src/

Up: [tf-compile/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `lib.rs` | `compile_to_ttf(path)`: compiles a `.ufo` or `.designspace` to TTF bytes (static or variable) with fontc's default options. `CompileError` covers an unsupported source extension, a fontc error, and a fontc panic caught and turned into an error |
