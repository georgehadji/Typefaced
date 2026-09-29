# ADR-0010: AI edits as sandbox proposals with three-way merge
Status: Accepted · Date: 2026-09-29

Source: [implementation plan](../implementation-plan.md) §2 (principle 4), §3.4, §5.12, §6.2, §6.7, §6.9, §7.3, §11.1, §14 (M1, M3, M5), Appendix A.

## Context

- Principle 4: AI proposes, the user decides (§2).
- An AI task can take many steps, and the user may keep editing while it runs (§5.12).
- Font data can carry prompt injections, so AI writes must be contained (§11.1).

## Decision

- **Every mutating AI task runs in a sandbox** (§5.12, §6.2):
  - at the start, the orchestrator forks a branch: `fork(doc) -> SandboxId`;
  - every mutating tool runs with `ExecCtx { origin: Ai, sandbox }`; read-only tools read the sandbox snapshot directly;
  - at the end, the UI requests `diff(sandbox)` and shows the **proposal**.
- **The user reviews, then merges or discards** (§5.12, §6.2). `merge(sandbox, selection)` applies all or part of the proposal as **one undo step**; `discard(sandbox)` drops it. The canvas shows the proposal as an overlay of ghost outlines with a colour-coded diff (§7.3).
- **Merging is a three-way merge at entity granularity** (glyph, kerning pair, feature block, info field), with conflict detection. It handles the case where the user edited the same glyph during an AI run (§5.12).
- **A failed multi-step task is compensated by discarding its sandbox** (saga via sandbox; §6.2).
- **External and destructive effects need confirmation** (§6.7, Appendix A). Commands of side-effect class E (writing outside the project, such as an export; installing fonts; using the network) and class D (bulk deletes, overwriting existing files) ask the user first.
- **MCP mutations use the same sandboxes** ([ADR-0012](0012-mcp-server-rmcp.md), §6.9).

## Consequences

- The user's document does not change until they accept a proposal, and one undo reverts an accepted proposal.
- Forks are cheap because state is persistent and structurally shared ([ADR-0003](0003-persistent-state-snapshot-undo-actor-rcu.md), §3.4).
- The engine needs entity-level diffs, merges and conflict reports (`ChangeSet`, `MergeReport`, `MergeError`; §5.12).
- The editor needs a proposal review with visual diffs (M5, §14).

## Alternatives considered

- The plan records no rejected alternative. For MCP clients only, the user can enable "trusted automation" for a session, which skips the approval (§6.9).

## Validation

- **Property tests** (§5.12): fork + discard leaves the base untouched; merge(all) equals applying the sandbox's commands to the base when nothing conflicts. The M1 exit criteria require the sandbox property tests to be green (§14).
- **M3 exit criterion** (§14): Claude Code (via MCP) and a minimal in-app chat can both add ligatures as a reviewable proposal.
- **Revisit** if entity granularity proves too coarse, for example when the user and the AI often change the same glyph at the same time.
