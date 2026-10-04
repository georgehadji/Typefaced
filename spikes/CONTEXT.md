# spikes/

Time-boxed experiments that decide an ADR. Each spike is its own Cargo workspace with its
own `Cargo.lock`, so it stays out of the root workspace, its lockfile and product CI. Run
a spike with `--manifest-path spikes/<name>/Cargo.toml`. Results are written up in
`docs/spikes/`. Up: [root CONTEXT.md](../CONTEXT.md).

| Subfolder | Context |
|---|---|
| `spike-boolean/` | [CONTEXT.md](spike-boolean/CONTEXT.md): Spike 5, boolean engine for overlap removal (ADR-0008) |
| `spike-collections/` | [CONTEXT.md](spike-collections/CONTEXT.md): Spike 6, persistent collections (ADR-0017) |
