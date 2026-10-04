# crates/tf-compile/tests/data/

Input files for the integration tests. Up: [tests/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `discrete-axis.designspace` | Designspace 5 with one discrete axis (`ital`, values 0 and 1) and a single source, `tests/fixtures/min.ufo`. fontc panics on it; `compile_fixture.rs` uses it to check the panic becomes an error |
