# ADR-0015: Parametric glyph engine as the backbone of AI glyph generation
Status: Accepted · Date: 2026-09-29

Source: [implementation plan](../implementation-plan.md) §3.4, §5.7, §6.1, §6.6, §6.8, §11.3, §11.4, §14 (M6), §15 (R4, R6), Appendix C.6; [research](../research.md) §7.1.

## Context

- "Font from a description" and the glyph completer are 1.0 playbooks (§6.1).
- In the VecGlypher benchmark, general-purpose LLMs reached only 24–47% relative accuracy when drawing glyph outlines directly (research §7.1). They are strong at choosing parameters, following constraints and judging images (§6.6).

## Decision

- **The AI steers a parametric glyph engine** (`tf-param`) and critiques rendered proofs. It does not draw outlines (§3.4, §6.6).
- **`tf-param` generates outlines from declarative recipes** (§5.7):
  - skeleton paths are expressions of metrics and style parameters;
  - strokes are assigned a nib (stroke models: monoline, broad nib, pointed pen);
  - glyphs reuse shared parts: stems, bowls, arches, serifs.
- **The pipeline** (§5.7): evaluate expressions → stroke expansion and curve fitting → terminals and serifs → boolean union → clean-up → write the glyph layer, with the recipe and parameters recorded in `lib`.
- **It fits parameters to seed glyphs** the user drew: measure them, start from those measurements, then refine with derivative-free optimisation (Nelder–Mead or CMA-ES) that minimises the raster difference (§5.7).
- **Recipes are data** in `assets/recipes/`. They are authored from scratch and never traced from existing fonts. Every generated glyph records its provenance (§5.7, §11.4).
- **Editing a generated glyph by hand detaches it** from its recipe. Re-attaching shows a diff first (§5.7).
- **Generation runs in a sandbox** and ends as a proposal ([ADR-0010](0010-ai-edits-as-sandbox-proposals.md), §6.6).
- **1.0 scope** (§5.7): basic Latin (A–Z, a–z, 0–9, common punctuation); three skeleton families (grotesque sans, humanist sans, slab); accented letters built from components and anchors.

## Consequences

- Output is consistent and editable (§3.4).
- Generated glyphs and starter templates come from Typefaced's own recipes, so they involve no third-party font data and carry no third-party license (§11.3, §11.4).
- Recipe design needs type-design expertise (R6). Mitigations: a small scope, recipes as data, a paid review by a type designer, and visual evals (§5.7).
- Generation quality may disappoint (R4). Mitigations: the parametric backbone, the visual critique loop, evals, human review and the basic-Latin scope.
- The union step depends on the boolean engine ([ADR-0008](0008-boolean-engine.md)).

## Alternatives considered

- **The LLM draws raw outlines.** Rejected: LLMs draw glyphs poorly (§3.4, research §7.1).
- **Image diffusion plus tracing.** Rejected as the backbone (§3.4). Optional third-party image-model adapters (with the user's own keys) plus tracing are an after-1.0 candidate, with each model's license checked (§6.6).

## Validation

- **Glyph-generation eval suite** (§6.8): measured consistency metrics, a legibility read-back and periodic human rating.
- **Recipe golden path** (Appendix C.6): every recipe gets render snapshot tests at three parameter settings and consistency-metric thresholds.
- **M6 exit criterion** (§14): a beginner makes an installable font in under 15 minutes (usability test).
- **Revisit** if the glyph-generation evals or the M6 usability test miss their targets.
