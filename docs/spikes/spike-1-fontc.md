# Spike 1: fontc as an in-process library

Validates [ADR-0006](../adr/0006-compiler-fontc-in-process.md). Plan: [M0 Step 6](../../plans/typefaced-m0-foundations-and-spikes.md). Code: [`crates/tf-compile/`](../../crates/tf-compile/).

## Question

Can fontc run in-process on Windows, linked into the app, with acceptable build time, binary size and compile speed? Which designspace features fail?

Pass criteria (ADR-0006, Step 6 exit criteria):
- in-process compiles succeed for the fixture and both corpus families, and every output passes OTS;
- cold build time, binary size delta and median compile time for the static family are recorded, against the §10.4 budget of under 2 s for a 1,000-glyph static TTF;
- unsupported designspace features are listed.

## Setup

| Item | Value |
|---|---|
| CPU | Intel Core i7-9750H, 6 cores / 12 threads, 2.6 GHz base (laptop) |
| RAM | 32 GB |
| OS | Windows 11 Pro, build 26300 |
| Toolchain | rustc 1.98.1 (48a229cea 2026-09-01), cargo 1.98.1; root `[profile.release]`: `lto = true`, `codegen-units = 1`, `opt-level = 3`, `strip = true` |
| Crates | `fontc` =1.0.0 (MIT/Apache-2.0, `default-features = false`, feature `rayon`), which brings `fontbe`/`fontir`/`ufo2fontir`/`glyphs2fontir`/`fontdrasil` 1.0.0, `fea-rs` 1.0.0, `fontra2fontir` 0.5.0, `write-fonts` 0.52.0, `read-fonts` 0.43.3, `norad` 0.18.4, `kurbo` 0.13.1. Dev: `skrifa` =0.46.2 (the version in fontc's tree; its `raw` module is `read-fonts`). |
| Validators | fontTools 4.61.1 (`ttx`), opentype-sanitizer 9.2.0 (`python -m ots`, which runs the bundled `ots-sanitize.exe`) |
| Corpus | `tests/corpus/manifest.toml`: `inria-sans` (static, 6 UFOs of 591 glyphs), `source-sans-upright` (variable, 3 masters of 2,494 / 237 sparse / 2,498 glyphs), `fontc-testdata` |
| Load | **The machine was saturated throughout**: two other agent sessions were building Rust and Node projects at the same time (CPU load 100% when sampled). Every time below is an upper bound; see *Results* for how much it matters. |

**MSRV.** fontc 1.0.0 declares no `rust-version`; it uses edition 2024 (Rust 1.85 or later). It builds on the pinned 1.98.1, so the toolchain stays as it is.

**API (docs.rs and source of fontc 1.0.0).** `fontc::Input::new(&Path) -> Result<Input, fontc::Error>` picks the reader from the extension (`.ufo` and `.designspace` both use `DesignSpaceIrSource`); `Input::create_source() -> Result<Box<dyn Source>, Error>`; `fontc::generate_font(Box<dyn Source>, Options) -> Result<Vec<u8>, Error>`. `Options::default()` uses `Flags::default()` = `PREFER_SIMPLE_GLYPHS | PRODUCTION_NAMES`, the same as the command line without flags.

**Licenses.** The PR adds 71 packages to `Cargo.lock`; `cargo deny check` passes with no allow-list change (all MIT, Apache-2.0, BSD, ISC, Zlib or Unicode, as `OR` or `AND` combinations). Advisories: see *quick-xml advisories* below.

## Method

- **Adapter.** `tf_compile::compile_to_ttf(&Path) -> Result<Vec<u8>, CompileError>`: rejects anything but `.ufo`/`.designspace`, then calls fontc with default options inside `std::panic::catch_unwind` (see *Panics*). Errors are `UnsupportedSource`, `Fontc(fontc::Error)` and `Panicked(String)`.
- **Fixture test** (`tests/compile_fixture.rs`, runs in CI): compiles `tests/fixtures/min.ufo`, then with `read-fonts`/`skrifa`: `maxp` has 6 glyphs; `cmap` maps U+0041 to `A` and U+00C1 to `Aacute` (by `post` names); `glyf` exists; `A` draws with 2 contours.
- **Corpus tests** (`tests/corpus.rs`, `tests/designspace_v5.rs`, `#[ignore]`): every Inria Sans style compiles with as many glyphs as the source; the Source Sans variable designspace fails as-is (see below) and compiles with a `wght` axis and `gvar` once its `features.fea` files are left out; a table of designspace 5 cases pins what fontc does with each.
- **Build and size.** `CARGO_TARGET_DIR=target/spike1 cargo build --release -p tf-compile --example compile --timings` in an empty target directory (crate sources already downloaded), then `--example baseline` in the same directory. `baseline` has the same shape as `compile` (argument parsing, file I/O, timing) but never calls fontc.
- **Speed and resources.** The release `compile` example was run 5 times per input as a fresh process. The time is the example's own measurement around `compile_to_ttf` (source reading included, process start-up excluded); CPU time and peak working set come from the process. Outputs were checked with `ttx -l` and OTS.

Reproduce:

```bash
cargo test -p tf-compile
cargo xtask corpus fetch && cargo test -p tf-compile -- --ignored
CARGO_TARGET_DIR=target/spike1 cargo build --release -p tf-compile --example compile --example baseline
target/spike1/release/examples/compile tests/fixtures/min.ufo target/min.ttf     # prints milliseconds
ttx -l target/min.ttf && python -m ots target/min.ttf
```

## Results

### Pass criteria

| Criterion | Result |
|---|---|
| Fixture compiles in-process, checks pass | Yes |
| Static family (Inria Sans, 6 styles) compiles | Yes, all 6, glyph counts equal the sources |
| Variable family (Source Sans 3 VF) compiles | **Not as-is**: fontc stops with `NonIdenticalFea` because the three masters have different `features.fea` files. With the `features.fea` files left out (kerning and marks are still generated from the UFO data) it compiles: `wght` axis, `gvar`, `HVAR`, `MVAR`, `avar`, `STAT` |
| Every output passes OTS | Yes, all 9 outputs (`File sanitized successfully!`); `ttx -l` reads all of them |
| Cold build time | 42 min 54 s on the saturated machine, 25 min of it in the final LTO link (see below) |
| Binary size delta | **14.4 MB** (13.7 MiB) |
| Static compile time, ~1,000 glyphs | 0.99 s at 591 glyphs with features, ~1.7 s per 1,000 glyphs by scaling; **within the 2 s budget**, with little margin on this loaded machine |
| Unsupported designspace features listed | Yes, see below |

### Build time and binary size

| Measure | Value |
|---|---|
| Cold `cargo build --release --example compile` | 2,574 s wall (42 min 54 s), 12 jobs, 207 units, 7,154 s summed unit time |
| of which: final link of the example (fat LTO, 1 codegen unit) | 1,496 s |
| slowest crates | `fontbe` 596 s, `write-fonts` 499 s, `fea-rs` 297 s, `norad` 281 s, `ufo2fontir` 232 s, `glyphs-reader` 219 s, `read-fonts` 214 s |
| `compile.exe` (release, stripped) | 14,535,168 bytes |
| `baseline.exe` | 139,776 bytes |
| **Delta** | **14,395,392 bytes = 14.4 MB (13.7 MiB)** |

The build time is dominated by the release profile, not by fontc as such: with fat LTO and one codegen unit, the final link optimises the whole program on one thread. The desktop app pays this once per release build; debug builds and `cargo test` do not use LTO. The size includes readers fontc links in even though `tf-compile` never uses them: Glyphs (`glyphs-reader`, `glyphs2fontir`) and Fontra (`fontra2fontir`), because `fontc::Input` names them and fontc has no feature to leave them out.

### Compile speed, CPU and memory (release, 5 runs each, saturated machine)

| Input | Glyphs | Runs (ms) | Median | CPU time (median run) | Peak RAM | TTF size |
|---|---:|---|---:|---:|---:|---:|
| `min.ufo` fixture | 6 | 78, 447, 149, 430, 426 | 426 ms | 0.28 s | 14.5 MB | 1 KB |
| Inria Sans Regular | 591 | 951, 1007, 1089, 1367, 1189 | 1,089 ms | 1.52 s | 37 MB | 58 KB |
| Inria Sans Italic | 591 | 910, 1453, 1363, 1122, 1456 | 1,363 ms | 1.48 s | 37 MB | 60 KB |
| Inria Sans Light | 591 | 1102, 1526, 1382, 2480, 1046 | 1,382 ms | 1.56 s | 36 MB | 58 KB |
| Inria Sans Light Italic | 591 | 944, 979, 1074, 842, 855 | 944 ms | 1.72 s | 36 MB | 59 KB |
| Inria Sans Bold | 591 | 836, 890, 799, 924, 910 | 890 ms | 1.61 s | 37 MB | 57 KB |
| Inria Sans Bold Italic | 591 | 1534, 955, 764, 906, 826 | 906 ms | 1.67 s | 37 MB | 59 KB |
| Source Sans ExtraLight master as a static UFO, no `features.fea` | 2,494 | 3350, 4880, 4238, 4681, 4135 | 4,238 ms | 6.41 s | 100 MB | 336 KB |
| Source Sans 3 VF, 3 masters, no `features.fea` | 2,496 | 5988, 8258, 8492, 7817, 9691 | 8,258 ms | 11.78 s | 123 MB | 614 KB |

- **Static family median: 993 ms** over all 30 Inria Sans runs (591 glyphs, features, kerning and marks). Scaled linearly to 1,000 glyphs: about 1.7 s. The 2,494-glyph Source Sans master gives the same rate (1.7 ms per glyph). Both are under the 2 s budget, but with only about 15% margin, and on a machine whose 12 threads were busy with other builds: the 6-glyph fixture took 78 ms on its first run and over 400 ms on later ones, which is load, not work. The budget should be re-checked on an idle machine (follow-up).
- fontc uses several threads: CPU time is 1.1–1.8 × wall time even on the loaded machine.
- Memory is modest: under 40 MB for 591 glyphs, 123 MB for the 2,500-glyph variable family.

### Designspace features

Each case is a format 5.0 designspace over the two `WghtVar` UFOs of `fontc-testdata` (`tests/designspace_v5.rs` pins the outcomes).

| Feature | fontc 1.0.0 |
|---|---|
| `format="5.0"` with continuous axes only | Compiles |
| **Discrete axis** (`<axis values="0 1" …>`, no `minimum`/`maximum`) | **Panics**: `called Option::unwrap() on a None value` in `ufo2fontir::toir::to_ir_axis` |
| **`<variable-fonts>`** (several fonts from one document) | Ignored: one font is built from all axes; subsets are not applied. With a discrete axis it panics as above |
| **Instance location in user coordinates** (`<dimension uservalue=…>`) | **Panics** inside a fontc job (`A task panicked: called Option::unwrap() on a None value`) |
| **avar2 axis mappings** (`<mappings>`) | Ignored: no `avar` table is written |
| **Axis labels** (`<labels>` in `<axis>`) and `elidedfallbackname` | Ignored: `STAT` gets no axis values |
| Location labels (document-level `<labels>`) | Ignored (no effect on the binary) |
| Masters with **different `features.fea` files** (any format) | Error `NonIdenticalFea`; fontc's own log calls it "an unnecessary limitation". This is why Source Sans 3 VF fails |

### Other findings

- **Panics.** fontc catches panics inside its own jobs (`fontc::Error::Panic`), but a panic while it sets up the work (the discrete-axis case) reaches the caller. `compile_to_ttf` therefore wraps the call in `catch_unwind` and returns `CompileError::Panicked`. This only works while the release profile keeps `panic = "unwind"`; if crash handling later picks `abort`, compiles need a process boundary.
- **norad rejects XML comments inside `<glyph>`.** The hand-made fixture had comments inside `<outline>`; norad 0.18.4 fails with `unexpected element`. fontTools accepts them. The comments moved to the XML prolog (same geometry). Typefaced's own UFO writer must not emit comments inside `<glyph>`, and opening third-party UFOs with such comments will fail until norad changes.
- **quick-xml advisories.** `norad` 0.18.4 depends on `quick-xml` ^0.38, which has two advisories fixed only in 0.41: RUSTSEC-2026-0194 (quadratic time on a start tag with many attributes) and RUSTSEC-2026-0195 (unbounded memory for namespace declarations in `NsReader`, which quick-xml's serde deserializer uses). There is no norad release on quick-xml 0.41 yet (0.18.4 is the latest). The impact for Typefaced is denial of service when the user opens a crafted UFO or designspace (CPU or memory exhaustion), not code execution. `deny.toml` ignores both IDs with that reason; they must be removed when norad updates.
- **Source Sans 3 is an AFDKO project.** Its masters' `features.fea` files include files outside the pinned `Upright/` folder (`../../../featuresVF.fea`, `familyOS2.fea`, instance `kern.fea`), so even fontmake could not build features from the pinned corpus alone. The no-features runs still exercise kerning and mark positioning, which fontc generates from the UFO data.

## Decision

**ADR-0006 is Accepted** (status changed in the same PR). fontc 1.0.0 runs in-process on Windows through a ~20-line adapter, every output passes OTS, the static compile rate (~1.7 s per 1,000 glyphs, measured on a saturated machine) is within the 2 s budget, and memory is small. The variable family compiles once its per-master feature files are removed.

Conditions that come with it:
- Typefaced writes the temporary UFO + designspace for fontc (ADR-0006) as **designspace 4 features only**, with **one shared `features.fea`** for all masters, and no XML comments inside glyphs.
- Every compile goes through `compile_to_ttf` (or the M2 port), which turns fontc panics into errors.
- The quick-xml advisories are tracked until norad moves to quick-xml 0.41 or later.

## Follow-ups

- Re-time the static family on an idle machine before M2 sets the export budget in CI; the margin under 2 s is small on this run.
- M2 `FontCompiler` port: keep panic capture; decide whether compiles run in a worker process if the release profile switches to `panic = "abort"`.
- Release build time: the 25-minute fat-LTO link on this machine suggests `lto = "thin"` or more codegen units for local release builds; decide with the packaging work.
- Watch norad for a release on quick-xml ≥ 0.41, then drop the two `deny.toml` ignores.
- Report upstream (fontc): panics on discrete axes and on instance user-space locations, and the `NonIdenticalFea` limitation. Not done from this spike.
- Corpus: add a variable family whose masters share one feature file, so a full-feature variable compile is covered (Step 7 or M2).
- New commands for Step 12 to fold into CLAUDE.md: `cargo test -p tf-compile -- --ignored` (corpus-backed compiler tests); `cargo run --release -p tf-compile --example compile -- <in> <out.ttf>`; OTS check `python -m ots <font>` (after `pip install --user opentype-sanitizer`).
