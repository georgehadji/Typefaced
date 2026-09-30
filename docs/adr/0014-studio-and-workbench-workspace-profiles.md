# ADR-0014: Studio and Workbench as workspace profiles over one engine
Status: Accepted · Date: 2026-09-29

Source: [implementation plan](../implementation-plan.md) §0, §1 (D2), §5.6, §5.11, §7.4, §9, §14 (M6, M7).

## Context

- D2: Typefaced serves beginners and professionals, with one engine and one document model (§1).
- The two audiences need different things (§0):
  - beginners (Studio): templates, "font from a description", handwriting import, auto-spacing, one-click export;
  - professionals (Workbench): kerning groups, feature code, masters and variable fonts, imports from other editors.

## Decision

- **One engine, one document, two workspace profiles** (§7.4). Profiles are configuration objects (the Strategy pattern). Each profile decides:
  - which panels and tools are visible;
  - the command-palette entries (`when` clauses in the UI command registry);
  - default AI playbooks;
  - terminology ("letter spacing" vs. "sidebearings");
  - guardrails: Studio hides masters and raw feature code, and uses managed rules;
  - defaults, such as auto-spacing after generating glyphs.
- **Switching workspace is instant and never converts data.** Panels offer progressive disclosure ("Show advanced") (§7.4).
- **Each command descriptor lists the workspaces that expose it** (§5.11).
- **Studio flows are wizards,** each a state machine plus step components. **Workbench** offers the professional panels (§7.4).
- **The default project format differs:** the `.typefaced` package in Studio, a UFO folder project in Workbench (§9, [ADR-0005](0005-native-formats-ufo-designspace-typefaced-package.md)).
- **Managed rule blocks** (Studio) and user `.fea` code (Workbench) share one feature source, separated by markers (§5.6).

## Consequences

- A project made in Studio opens in Workbench without conversion, and the other way round.
- Every panel, tool and command needs a visibility decision for each profile.
- Studio's guardrails rely on managed rule blocks. Generated code never overwrites user code outside the markers, and a user who edits inside a block turns it into user code after an explicit confirmation (§5.6).

## Alternatives considered

- The plan records no rejected alternative. D2 fixes one engine and one document model (§1).

## Validation

- **Review rule:** a workspace is configuration over shared panels and tools. It never adds document data or an engine path of its own (D2, §7.4).
- **Milestone exits** (§14):
  - M6: a beginner makes an installable font in under 15 minutes (usability test);
  - M7: an existing family is edited and exported as static OTF/TTF plus a variable font, matching fontmake within tolerance.
- **Revisit** if usability tests show that one audience needs a different document model or engine behaviour.
