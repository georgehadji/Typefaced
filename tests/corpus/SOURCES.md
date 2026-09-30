# Corpus sources

The font corpus is not committed. `cargo xtask corpus fetch` downloads each source
listed in [`manifest.toml`](manifest.toml) into the git-ignored `.cache/` directory.
The files are used only for tests and benchmarks and are never shipped with Typefaced.
Each license was checked in the repository at the pinned commit.

| Name | Source (pinned commit) | Path | License | Notes |
|---|---|---|---|---|
| `fontc-testdata` | [googlefonts/fontc @ `e62f581`](https://github.com/googlefonts/fontc/tree/e62f5818357f796f329f9bc1335c0e14a104ce09) (tag `fontc-v1.0.0`) | `resources/testdata` | Apache-2.0 ([LICENSE](https://github.com/googlefonts/fontc/blob/e62f5818357f796f329f9bc1335c0e14a104ce09/LICENSE)) AND OFL-1.1 | The repository is Apache-2.0. Some samples are reduced copies of OFL-1.1 fonts (Oswald, Noto Serif CJK JP), so their OFL-1.1 terms also apply. The `.fontra` files are font data, not Fontra source code. |
| `inria-sans` | [BlackFoundryCom/InriaFonts @ `95a529c`](https://github.com/BlackFoundryCom/InriaFonts/tree/95a529cee87d8be884632644d3d37ad28512be28) | `masters/INRIA-SANS` | OFL-1.1 ([OFL.txt](https://github.com/BlackFoundryCom/InriaFonts/blob/95a529cee87d8be884632644d3d37ad28512be28/OFL.txt)) | Copyright 2017 The Inria Sans Project Authors. |
| `source-sans-upright` | [adobe-fonts/source-sans @ `272b22b`](https://github.com/adobe-fonts/source-sans/tree/272b22b02e097e8eff1372111f88b5ab6063499f) | `Upright` | OFL-1.1 ([LICENSE.md](https://github.com/adobe-fonts/source-sans/blob/272b22b02e097e8eff1372111f88b5ab6063499f/LICENSE.md)) | Copyright 2010-2024 Adobe, with Reserved Font Name 'Source'. Fonts built from it for tests must not use that name. |

Adding a source: pick a commit, read its license at that commit (only OFL-1.1,
Apache-2.0 and MIT are accepted), add a `[[entry]]` with any 64-hex digest, run
`cargo xtask corpus fetch` and copy the real digest from the mismatch message.
