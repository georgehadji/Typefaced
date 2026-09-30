# ADR-0018: Hinting: ttfautohint sidecar for TTF only in 1.0
Status: Accepted · Date: 2026-09-29

Source: [implementation plan](../implementation-plan.md) §3.3, §5.8, §5.10, §5.13, §11.3, §13.3, §14, Appendix B; [research](../research.md) §6.1, §7.2.

## Context

- fontc 1.0 does no hinting (research §6.1).
- ttfautohint is dual-licensed under the FreeType License and GPLv2. It handles static TrueType only, and it has been dormant since 2021 (version 1.8.4). otfautohint (in AFDKO, Apache-2.0) handles CFF and CFF2. No tool autohints variable fonts (research §7.2).
- There is no Python at runtime (§3.3).

## Decision

- **Hinting is optional and applies to TTF only in 1.0:** TTF → ttfautohint (§5.13, stage 7).
- **ttfautohint runs as a short-lived command-line sidecar**, currently the app's only sidecar (§3.3), behind the `Hinter` port (§5.10).
- **It is used under the FreeType License** (§5.13). The FreeType License credit goes into the documentation (§11.3, Appendix B).
- **CFF output is unhinted in 1.0** (§5.8, §5.13). CFF hinting is on the after-1.0 backlog (§14).

## Consequences

- Variable TTFs stay unhinted, because ttfautohint handles static TrueType only (research §7.2).
- The sidecar ships with the app and runs as a separate process.
- On Windows ARM64, ttfautohint runs under x64 emulation for now (§13.3).
- Upstream has been dormant since 2021, so fixes may not come. The `Hinter` port keeps it replaceable.

## Alternatives considered

- **otfautohint for CFF.** Listed as the future alternative behind the `Hinter` port (§5.10). Not in 1.0: it runs through Python, and there is no Python at runtime (§3.3).

## Validation

- **M2** (§14) adds the ttfautohint sidecar. Its exit criteria include corpus fonts building to TTF, OTF and WOFF2, with fontspector's critical checks passing.
- **Notices:** the FreeType License credit is part of the documentation and third-party notices ([ADR-0016](0016-proprietary-license-dependency-policy-clean-room.md)).
- **Revisit** when CFF hinting is planned after 1.0, or if a maintained autohinter for CFF or variable fonts becomes available without Python.
