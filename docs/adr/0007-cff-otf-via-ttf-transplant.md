# ADR-0007: CFF-based OTF via TTF → CFF transplant with `tf-cff`
Status: Accepted · Date: 2026-10-03

Source: [implementation plan](../implementation-plan.md) §1 (D4), §3.4, §5.8, §5.10, §5.13, §10.4, §14, §15 (R2); [research](../research.md) §6.1; [M0 plan](../../plans/typefaced-m0-foundations-and-spikes.md) Step 7.

## Context

- Typefaced must ship CFF-based OTF (D4), but fontc emits only TrueType outlines (Step 7).
- fontc's TTF already holds the tables that don't depend on outlines, such as cmap, name, OS/2, GSUB and GPOS (§5.13, stage 6).
- The sources are cubic and follow the PostScript contour direction, which is what CFF expects (Step 7).

## Decision

- **Transplant a `CFF ` table into fontc's TTF** (§5.13, stage 6):
  - keep everything that doesn't depend on outlines: cmap, name, OS/2, hhea, hmtx advances, GSUB, GPOS, GDEF, STAT;
  - drop the TrueType outline and instruction tables: `glyf`, `loca`, `cvt `, `fpgm`, `prep`, `hdmx`, `LTSH`, `VDMX`, `gasp` (Step 7);
  - set `maxp` to version 0.5 and `post` to version 3.0;
  - build `CFF ` from the original **cubic** outlines, not from fontc's quadratic ones, in fontc's glyph order;
  - recompute the `head` bounding box, `hmtx` left side bearings and `hhea` extents from the cubic bounds;
  - set the sfnt version to `OTTO` and recompute the checksums.
- **Decompose composite glyphs** before they reach the CFF writer: CFF has no components, and seac-style accents are never used (Step 7).
- **`tf-cff` is a pure serialiser** for CFF version 1: domain layer, data in, bytes out, no file access (§5.8).
  - A builder API, a Type 2 charstring encoder, typed INDEX and DICT structures before bytes, and optimisation levels as a Strategy.
  - In 1.0: empty subroutine INDEXes and no hints.
- **It sits behind the `CffWriter` port** (§5.10). There is no allsorts-based alternative: allsorts cannot build a CFF table for a new font (Spike 2).
- **Build, not reuse** (decided by Spike 2). Neither `write-fonts` 0.52.0 (no CFF 1 writer compiled in) nor `allsorts` 0.17.0 (no public way to create INDEX or DICT entries, no outline-to-charstring encoder) can serialise CFF for new fonts. The writer took about 650 lines of Rust (estimate: 1.5–2.5k, §5.8).
- **Variable fonts ship as TTF.** CFF2 is not planned (§5.8).

## Consequences

- OTF export needs no Python at runtime (§3.4). Budget: a static OTF of 1,000 glyphs in under 3 s (§10.4).
- A bug in the CFF writer produces broken OTFs (R2). Three independent checks run on every change in CI (§5.8):
  1. a round-trip parse with `read-fonts` and outline extraction with `skrifa`, which must equal the rounded source outlines exactly;
  2. `ttx` (fontTools) and OpenType Sanitizer, in CI only;
  3. a rendering comparison against a fontmake-built OTF at several sizes.
- CFF output has no subroutines and no hints in 1.0, so files are bigger than they need to be. Subroutinisation and CFF hinting are on the after-1.0 backlog (§5.8, §14).

## Alternatives considered

- **Bundled Python fontmake for OTF** (research §6.1 suggested it as an optional sidecar). Rejected: it means shipping a ~60 MB Python runtime and paying its start-up cost on every export. fontmake stays as the test oracle and a fallback adapter (§3.4).
- **Reusing `allsorts` or `write-fonts`** for CFF serialisation. Evaluated by Step 7 before any writer code is built.

## Validation

**Validated by Step 7 — Spike 2: CFF writer and TTF→OTF transplant.** Step 7 converts the fixture and the corpus static family to OTF, compares every glyph's outline with the decomposed cubic source, and checks the result with `ttx`, OpenType Sanitizer and Windows Font Viewer.

Pass criteria (Step 7 exit criteria):
- For the fixture and the corpus static family:
  - OTF outlines are identical to the rounded, decomposed source (0 point differences);
  - OTS passes;
  - `ttx` parses the file;
  - Windows Font Viewer renders it.
- The build-versus-reuse decision is made, with evidence.

Validated by Step 7 — Spike 2 ([report](../spikes/spike-2-cff.md)), with `tf-cff` and `tf_compile::ttf_to_otf` on fontc 1.0.0's TTF:
- **Outlines:** 0 point differences against the rounded, decomposed source for the fixture (6 glyphs, `Aacute` = `A` + `acute` at the component offset) and all 6 Inria Sans styles (591 glyphs each, 312 composites).
- **Validators:** OTS and `ttx` (the `CFF ` table and a full dump) pass for all 7 fonts; fontTools recomputes the same `FontBBox`, `head` box, left side bearings and `hhea` extents.
- **Windows Font Viewer** renders all 6 Inria Sans OTFs from the file, without installing them (screenshot in the report).
- **Build vs reuse:** build (see *Decision*).
- **Speed:** `compile_to_otf` takes about 0.78 s for 591 glyphs on a release build without fat LTO (about 1.3 s per 1,000 glyphs; budget 3 s, §10.4).

Known gaps: no subroutinisation, no hints, one charstring operator per segment (OTFs about 40% larger than the TTFs), no Top DICT FontInfo keys, no `vmtx`, `.ufo` sources only. The report lists them as follow-ups.
