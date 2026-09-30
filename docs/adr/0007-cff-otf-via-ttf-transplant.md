# ADR-0007: CFF-based OTF via TTF → CFF transplant with `tf-cff`
Status: Proposed · Date: 2026-09-29

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
- **It sits behind the `CffWriter` port** (§5.10), with an allsorts-based adapter as the alternative.
- **Build or reuse is decided by the spike.** Step 7 first checks whether `write-fonts` or `allsorts` can serialise CFF for new fonts; reuse wins if it meets the pass criteria with less code. Writing it from scratch is estimated at 1.5–2.5k lines of Rust (§5.8).
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

Step 7 sets this ADR to Accepted or Rejected, recording the build-versus-reuse decision and the known gaps (no subroutinisation, no hints). If it is rejected, the fallback is a fontTools/fontmake CFF path as a sidecar. That brings Python back at runtime, so it needs the user's approval as an amendment to this ADR.
