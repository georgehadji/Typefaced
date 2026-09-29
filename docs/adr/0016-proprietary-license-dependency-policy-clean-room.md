# ADR-0016: Proprietary license, dependency policy and clean-room rule
Status: Accepted · Date: 2026-09-29

Source: [implementation plan](../implementation-plan.md) §1 (D1, repository note), §2 (principle 9), §11.1, §11.3, §15 (R8), §17, Appendix B; [research](../research.md) §8; [M0 plan](../../plans/typefaced-m0-foundations-and-spikes.md) Conventions, deviation 7, Steps 2, 3.1 and 3.2, change log; [`LICENSE`](../../LICENSE); [`CLAUDE.md`](../../CLAUDE.md).

## Context

- D1: Typefaced is proprietary, all rights reserved (§1).
- Forking a GPL project, or combining its code in the same process, would make Typefaced GPL (research §8).
- AI-generated code can bring GPL code into the product (R8).
- On 2026-09-29 the user decided that the GitHub repository stays public (M0 plan change log).

## Decision

**License**
- `LICENSE` is a proprietary notice: all rights reserved.
- The repository is public, so the code is source-visible, not open source: anyone can read it, but nobody may reuse it (§1, repository note).
- Workspace crates declare `license = "LicenseRef-Proprietary"` and `publish = false` (Step 2).
- A lawyer reviews `LICENSE` before release. If outside contributions are ever accepted, a contributor license agreement (CLA) is added (§11.3).

**Dependency policy** (Appendix B)

| Category | Licenses | Condition |
|---|---|---|
| Allowed | MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, BSL-1.0, Unicode-3.0 / Unicode-DFS-2016, CC0-1.0 | Keep notices |
| Allowed with conditions | MPL-2.0 | Unmodified; file-level copyleft respected |
| | FTL (FreeType License) | Credit in the documentation |
| | OFL-1.1 | Fonts and assets only; license files kept |
| Denied | GPL-2.0/3.0, AGPL, LGPL, SSPL, EUPL, CC-BY-NC-\*, "non-commercial" or research-only model licenses, unlicensed or unknown | Any exception needs a written ADR and legal review |

- **Stop and ask.** Whoever meets a GPL, AGPL, LGPL, SSPL, EUPL, non-commercial or unknown license, human or agent, stops and asks the user before going further (M0 plan Conventions, `CLAUDE.md`).
- LGPL is excluded by default so the policy stays simple (Appendix B).
- A permissive license missing from the list (for example `0BSD`) may be added to `deny.toml`, with a comment giving the reason and the crate. Step 12 folds these additions into Appendix B (deviation 7, Step 3.1).
- AI models: only licenses that allow commercial use; no "non-commercial" weights (§11.3).
- Bundled fonts (the UI font, sample fonts) are OFL, and their license files are kept (§11.3).
- Third-party notices (§11.3): `THIRD_PARTY_NOTICES.txt` is generated per release (`cargo-about` plus an npm license generator) and shown in the About dialog; Apache-2.0 NOTICE files are propagated; the FreeType License credit for ttfautohint goes into the documentation.

**Clean-room rule** (§11.3, M0 plan Conventions, `CLAUDE.md`)
- Never read, copy or paraphrase the source code of the GPL projects Fontra, FontForge, Glyphr Studio, BirdFont, HT Letterspacer and potrace.
- Their documentation and UX may serve as inspiration only.
- Algorithms come from published descriptions. Specifications, papers and permissively licensed projects (fontc, fontations, allsorts, kurbo, fontTools) are fine.
- The rule lives in `CLAUDE.md` and the contributing guide (§11.3). [ADR-0019](0019-clean-room-spacing-algorithm.md) applies it to spacing.

## Consequences

- Copyleft libraries are off limits, even where they would save work.
- Every new dependency must pass the license gates before it lands.
- Every release carries third-party notices and a CycloneDX software bill of materials (§11.1, §11.3).
- Agents need the clean-room rule and the stop-and-ask list in their instructions; `CLAUDE.md` carries both.

## Alternatives considered

- **Allowing LGPL.** Excluded by default to keep the policy simple. An exception needs a written ADR and legal review (Appendix B).
- **A private repository.** The user chose to keep the repository public (§17 question 1, M0 plan change log).

## Validation

- **CI gates on every PR** (Steps 3.1 and 3.2): `cargo deny check`, with the Appendix B list as the allow-list in `deny.toml`, and `cargo xtask licenses-npm` for npm packages. Step 3.1 proves the gate works by removing `MIT` from the allow-list and confirming that `cargo deny check licenses` fails.
- **Review rule** (R8): the code-review checklist covers GPL contamination.
- **Release:** `xtask licenses` generates the third-party notices (Appendix B).
- **Revisit** if a needed capability exists only under a denied license (which needs a written ADR and legal review), or before outside contributions are accepted.
