# ADR-0004: Commands as the single API for UI, AI, MCP and CLI; generated types and schemas
Status: Accepted · Date: 2026-09-29

Source: [implementation plan](../implementation-plan.md) §0, §1 (D3), §2 (principles 1, 2), §5.11, §6.3, §6.9, §10.1, §15 (R9), Appendix A, Appendix C.1; [M0 plan](../../plans/typefaced-m0-foundations-and-spikes.md) Conventions, Invariants, Step 2.

## Context

- Four clients drive the engine: the UI, the in-app AI agent, the MCP server and the CLI (§0).
- D3 makes the architecture AI-first: every capability is a typed, schema-described command an agent can call (§1).
- The same catalog has to feed typed IPC, AI tool schemas, the MCP server and CLI subcommands (§5.11).

## Decision

- **`tf-commands` is the single, versioned catalog** of commands (mutations) and queries (reads). It holds types and metadata only, no logic (§5.11).
- **Every document change is a command:** typed, validated, undoable and auditable, with a JSON Schema (§2, principle 2). No client has a private back door (§2, principle 1).
- **A registry** holds one descriptor per command (§5.11):
  - identifier, title and description (also written for the AI), and examples;
  - side-effect class: Q query, M mutation, E external effect, D destructive (Appendix A);
  - which workspaces expose it;
  - whether it is AI/MCP-exposed, and whether it is parallel-safe.
- **Every request and response type derives** `serde`, `specta::Type` and `schemars::JsonSchema` (§5.11).
- **Code generation** from the registry emits (§5.11):
  - TypeScript types and IPC bindings (tauri-specta);
  - JSON Schemas for AI tools and MCP (schemars; strict-mode compatible: `additionalProperties: false`, all fields listed as `required`, optional fields as nullable);
  - CLI subcommands (clap).
- **The contract uses semver.** Deprecations carry replacement hints (§5.11).
- **Errors carry stable codes**, such as `E_GLYPH_NOT_FOUND`, so the UI, the AI agent and MCP clients can react programmatically (§10.1).
- New commands follow the golden path in Appendix C.1.

## Consequences

- Every new command is scriptable and callable by the AI and MCP without extra work (§2, §6.3).
- M commands run in a sandbox when the AI or MCP calls them; E and D commands need the user's confirmation (Appendix A, [ADR-0010](0010-ai-edits-as-sandbox-proposals.md)).
- Generated files, such as `packages/bindings/src/index.ts`, are never edited by hand (M0 plan Conventions).
- The catalog is also a public integration surface through MCP (§6.9), so breaking changes follow semver.
- tauri-specta is still a release candidate, so it is pinned exactly (R9, Step 2).
- IPC integer types stay 32-bit or smaller: specta refuses 64-bit integers by default (Step 2).

## Alternatives considered

- **A generic shell tool for the agent.** Rejected: dedicated tools can be gated, rendered, audited and scheduled in parallel (§6.3).
- **`ts-rs` plus generated invoke wrappers from our own registry**, instead of tauri-specta. Kept as the fallback (R9).

## Validation

- **Review rule** (`CLAUDE.md`): every document change is a typed command; no back doors for the UI, AI, MCP or CLI.
- **Step 2** proves the seam: a type in `tf-commands` is returned by a Tauri command, exported by tauri-specta into `@typefaced/bindings`, and rendered by the React UI.
- **CI invariant** (M0 plan Invariants, from Step 3.2): `git diff --exit-code packages/bindings` fails when the committed bindings are stale.
- **Contract tests** (§5.11): `insta` snapshots of the generated schemas catch accidental API breaks, and a contract test checks every AI-exposed schema against the strict-tool-schema limits of the Claude API.
- **Revisit** if tauri-specta stalls or breaks (R9), or if the Claude API's strict-schema limits change.
