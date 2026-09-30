# ADR-0005: Native formats: UFO 3 + designspace 5, and the `.typefaced` package; `com.typefaced.*` lib keys
Status: Accepted · Date: 2026-09-29

Source: [implementation plan](../implementation-plan.md) §0, §2 (principle 6), §5.13, §5.17, §9, §12.1, §14 (M1); [research](../research.md) §5.

## Context

- Principle 6: standard formats in and out, so users are never locked in (§2).
- Beginners need a single file they can share. Professionals need diff-friendly folders that work with git and other editors (§9).
- Typefaced keeps its own data per font and per glyph: provenance, recipes, style parameters, template layouts and managed features (§9).

## Decision

- **Two native project formats** (§9):

  | Format | Default in | Contents |
  |---|---|---|
  | UFO folder project | Workbench | `*.designspace` + `*.ufo` folders + `typefaced/` (assets) |
  | `.typefaced` package | Studio | A zip: `manifest.json` (format version, app version, project id), `font.designspace`, `masters/*.ufo`, `assets/` (scans, reference images), `ai/` (optional, opt-in conversation logs) |

- **Typefaced data lives under reverse-DNS keys** in UFO `lib.plist` (§9): `com.typefaced.provenance`, `com.typefaced.recipe`, `com.typefaced.styleParams`, `com.typefaced.templateLayout` and `com.typefaced.managedFeatures`.
- **Format versions** (§9): `manifest.formatVersion`, with one migration Strategy per version. A newer file opens read-only, with a warning, in an older app.
- **Deterministic output** (§9): stable ordering everywhere (`IndexMap`, sorted plist keys, canonical number formatting). Saving an unchanged project produces no diff.
- **`tf-io`** loads and saves both formats, using norad for UFO and designspace (§5.17). It:
  - saves incrementally: only changed `.glif` files are written, computed from the snapshot diff;
  - writes atomically: temporary file → fsync → rename;
  - detects external changes and supports crash recovery;
  - keeps norad types out of the domain (Anti-Corruption Layer).

## Consequences

- Folder projects are diff-friendly and interoperate with Fontra, FontForge, Glyphs (via glyphsLib), RoboFont and FontLab. Other editors keep the `com.typefaced.*` keys without understanding them (§9).
- The model follows designspace 5, but fontc compiles only the v4 feature level. Exports stay within v4 features until fontc supports v5 (§5.13, [ADR-0006](0006-compiler-fontc-in-process.md)).
- Packages are untrusted input: zip-slip protection, size limits and path normalisation apply, and `.fea` `include` statements resolve only inside the project (§5.17).
- Recovery data (the command journal plus periodic snapshots) lives outside the project, in `%LOCALAPPDATA%\Typefaced\recovery\<project-id>\` (§9).

## Alternatives considered

- The plan records no rejected native format. Other editors' formats (`.glyphs`, `.glyphspackage`, `.fontra`, `.sfd`) are import-only, through `tf-import` (§5.17).

## Validation

- **M1 exit criterion** (§14): a byte-stable UFO round-trip on the test corpus.
- **Golden tests** of UFO round-trips on corpus fixtures (§12.1).
- **Revisit** if other editors turn out to drop the `com.typefaced.*` keys in practice.
