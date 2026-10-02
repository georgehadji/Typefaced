# tests/corpus/

The pinned font corpus for tests and benchmarks. The fonts are not committed:
`cargo xtask corpus fetch` downloads each source into `.cache/` and `cargo xtask corpus
verify` checks the digests. Never shipped with Typefaced. Up: [tests/CONTEXT.md](../CONTEXT.md).

| File | What it does |
|---|---|
| `manifest.toml` | One `[[entry]]` per source: name, GitHub repo, pinned commit, path, license (OFL-1.1, Apache-2.0 or MIT only), SHA-256 digest, purpose. Sources: `fontc-testdata` (compiler edge cases), `inria-sans` (static UFO family), `source-sans-upright` (variable designspace) |
| `SOURCES.md` | Human-readable list of the sources with license links and notes, and how to add a source |
| `.gitignore` | Ignores `.cache/`, where the corpus is downloaded |
