# spikes/spike-collections/

Spike 6 (ADR-0017): which persistent collection holds the glyph table of a document
snapshot. Report: `docs/spikes/spike-6-collections.md`. Up: [spikes/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `Cargo.toml` | Own workspace root; depends on `imbl` (MPL-2.0) and `criterion` (benchmarks only); declares the `ops` and `memory` benches |
| `Cargo.lock` | Locked dependencies of this spike |

| Subfolder | Context |
|---|---|
| `src/` | [CONTEXT.md](src/CONTEXT.md): the four candidate glyph tables |
| `benches/` | [CONTEXT.md](benches/CONTEXT.md): timing and memory benchmarks |
