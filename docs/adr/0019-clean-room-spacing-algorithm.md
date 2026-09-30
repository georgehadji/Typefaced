# ADR-0019: Clean-room spacing algorithm from the published method
Status: Accepted · Date: 2026-09-29

Source: [implementation plan](../implementation-plan.md) §1 (D1), §5.5, §6.1, §6.8, §10.4, §11.3, §14 (M6), §15 (R8); [research](../research.md) §7.2, §8.

## Context

- Users expect one-click auto-spacing; FontLab 8 and Fontself already offer it (research §7.2).
- HT Letterspacer's area/depth/overshoot method is documented, and its documentation is CC BY 4.0. The plugin itself is GPL-3.0 and Glyphs-only (research §7.2).
- Copying or paraphrasing GPL code would make Typefaced GPL (research §8, [ADR-0016](0016-proprietary-license-dependency-policy-clean-room.md)).

## Decision

- **`tf-metrics` implements area-based spacing as a clean-room implementation of the published HT Letterspacer method,** alongside fixed, proportional and tabular spacing (§5.5).
- **Implement only from the published method description. Never read the GPL source** (§5.5).
- **How it is built** (§5.5):
  - data-oriented numerics over precomputed glyph profiles: per-scanline left and right extents, stored as structure-of-arrays;
  - Rayon parallelism across glyphs and pairs, with pure functions throughout;
  - a `SpacingAlgorithm` Strategy;
  - the auto-spacing workflow as a Template Method: categorise glyphs → measure reference glyphs → compute per glyph → round → propose.

## Consequences

- Auto-spacing ships without GPL exposure.
- The spacing and kerning assistant playbook runs this algorithm with chosen parameters and critiques rendered proofs (§6.1).
- Budget: auto-spacing a whole 1,000-glyph font takes under 1 s (§10.4).

## Alternatives considered

- **Porting or reusing the HT Letterspacer plugin.** Rejected: it is GPL-3.0 (research §7.2), and D1 allows no GPL code (§1).

## Validation

- **Tests** (§5.5): permissively licensed reference fonts give measured spacing baselines, and error metrics are checked against thresholds in CI. The spacing eval suite grades error against reference spacing (§6.8).
- **Review rule** (R8): the code-review checklist confirms that the spacing code comes only from the published description.
- **M6** (§14) ships auto-spacing v1.
- **Revisit** if the published description turns out to be insufficient. Reading the GPL source stays forbidden either way.
