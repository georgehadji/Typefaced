# ADR-0006: Compiler: fontc in-process; fontmake as oracle and contingency
Status: Proposed · Date: 2026-09-29

Source: [implementation plan](../implementation-plan.md) §0, §1 (D4), §3.3, §3.4, §5.10, §5.13, §10.4, §15 (R1); [research](../research.md) §5, §6.1; [M0 plan](../../plans/typefaced-m0-foundations-and-spikes.md) Conventions, Step 6.

## Context

- Typefaced exports static TTF, variable TTF, CFF-based OTF and WOFF2 (§0), with no Python at runtime (§3.3).
- fontc 1.0 (Rust; Apache-2.0/MIT) compiles UFO and designspace sources to static and variable TTF. It can be linked in-process as a library (research §6.1).
- fontc 1.x has no CFF output, no overlap removal, no hinting and no WOFF2, and supports designspace only at the v4 level (Step 6).
- The fontations crates are pre-1.0 and make breaking changes about monthly (R1, research §5).

## Decision

- **fontc, linked in-process, compiles TTF and variable TTF** (D4), behind the `FontCompiler` port (§5.10), in the adapter crate `tf-compile`. In M0 a small function wraps fontc; the formal port comes in M2 (Step 6).
- **Start with temporary files** (§5.13). Write a temporary UFO + designspace with norad and call fontc's library entry point (`fontc::generate_font`, confirmed in Spike 1). Later, implement fontc's `Source` trait on the snapshot, for builds without temporary files.
- **Stay within designspace v4 features** until fontc supports v5 (§5.13).
- **Fill fontc's gaps elsewhere:** CFF through the transplant ([ADR-0007](0007-cff-otf-via-ttf-transplant.md)), overlap removal through the boolean engine ([ADR-0008](0008-boolean-engine.md)), hinting through ttfautohint ([ADR-0018](0018-hinting-ttfautohint-sidecar.md)), and WOFF2 through `woofwoof` (§5.13).
- **fontmake is the test oracle and the contingency** (D4):
  - differential tests against fontmake run in CI, with Python installed in CI only (§5.13);
  - a fontmake sidecar behind the same port is the contingency adapter (§5.10).
- **Pin fontc and the fontations crates** with `=` versions (M0 plan Conventions). Upgrade them monthly, as one unit, with contract tests (R1).

## Consequences

- No ~60 MB Python runtime to ship, and no Python start-up cost on every export (§3.4).
- API churn is contained by pinning and by the port (R1).
- Designspace 5 features that fontc cannot compile yet are not available for export. Step 6 lists them.
- fontc 1.0 is not yet Google Fonts' default compiler; about 89% of about 3,450 Google Fonts builds come out identical to fontmake's after normalisation (research §6.1). The differential tests catch divergences.
- Export budget: a static TTF of 1,000 glyphs in under 2 s (§10.4).

## Alternatives considered

- **Bundled Python fontmake.** Rejected: it means shipping a ~60 MB Python runtime and paying its start-up cost on every export. fontmake stays as the test oracle and a fallback adapter (§3.4).

## Validation

**Validated by Step 6 — Spike 1: fontc as an in-process library.** Step 6 compiles the fixture UFO and two corpus families (one static, one variable), checks the outputs with `read-fonts`, `skrifa`, `ttx` and `ots-sanitize`, and measures build time, binary size and compile time.

Pass criteria (Step 6 exit criteria):
- In-process compiles succeed for the fixture and both corpus families, and every output passes OTS.
- Recorded: cold build time, binary size delta in MB, and median compile time for the static family. The budget in §10.4 is under 2 s at about 1,000 glyphs; if it's over, say by how much.
- Unsupported designspace features are listed.

Step 6 sets this ADR to Accepted or Rejected. If it is rejected, the fallback is a fontmake sidecar behind the same port. That changes a stated product constraint (no Python at runtime, §3.3), so it needs the user's approval as an amendment to this ADR.
