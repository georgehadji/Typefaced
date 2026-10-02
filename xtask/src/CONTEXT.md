# xtask/src/

Each module holds the pure, unit-tested logic of one task; `main.rs` does the I/O
(running cargo and pnpm, reading files, downloading). Up: [xtask/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `main.rs` | Parses the task name and dispatches; reads `cargo metadata`, runs `pnpm licenses`, `cargo llvm-cov`, downloads and extracts corpus archives, prints violations |
| `deps.rs` | `check-deps`: the `Layer` enum (domain, application, adapter, driver, tool), the inward-only dependency rule, banned external crates per layer (e.g. no Tauri or async runtime in domain), and the crate-template checks (layer declared, workspace fields and lints inherited) |
| `licenses.rs` | `licenses-npm`: reads the allow-list from `deny.toml`, maps common non-SPDX strings to SPDX, and checks every production npm package's license expression |
| `coverage.rs` | `coverage`: groups workspace crates by layer and builds the line-coverage gates (domain + application ≥ 90%, adapters ≥ 80%) and the `cargo llvm-cov` arguments |
| `corpus.rs` | `corpus fetch / verify`: parses and validates `tests/corpus/manifest.toml` (name format, `owner/name` repo, full 40-character commit SHA, safe relative path, allowed license, 64-hex digest), checks archive listings, computes the SHA-256 digest of a fetched source |
