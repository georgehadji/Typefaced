# ADR-0012: MCP server via `rmcp`; off by default; sandboxed writes
Status: Accepted · Date: 2026-09-29

Source: [implementation plan](../implementation-plan.md) §2 (principle 1), §4.1, §4.3, §6.7, §6.9, §11.1, §14 (M3, M8).

## Context

- Power users and AI-assisted workflows, including the developer's own, want to drive Typefaced from Claude Code and Claude Desktop. The MCP server also works as a public integration surface (§6.9).
- Abuse of the MCP server is a listed threat (§11.1).

## Decision

- **Library:** `rmcp`, the official Rust MCP SDK (MIT), in the driver crate `tf-mcp` (§6.9, §4.1).
- **Transports** (§6.9):
  - stdio, for Claude Desktop and Claude Code (`typefaced mcp`, headless);
  - streamable HTTP on localhost only, while the app is open.
- **Tools come from the same command registry** as the in-app agent ([ADR-0004](0004-commands-as-single-api.md), §6.9).
- **Off by default** (§6.9):
  - each client needs its own bearer token;
  - read-only clients can be granted separately.
- **Sandboxed writes:** every mutation lands in a sandbox that needs approval in the app ([ADR-0010](0010-ai-edits-as-sandbox-proposals.md)), unless the user enables "trusted automation" for that session (§6.9).
- **The AI kill switch also disables the MCP server** (§6.7).

## Consequences

- External agents work under the same rules as internal ones (§4.3).
- Open point: approval happens in the app, but the stdio transport runs headless (`typefaced mcp`). The plan does not yet say how a headless session gets its sandboxes approved; `tf-mcp` must settle this when it is built in M3.
- Tokens, the localhost binding and read-only grants are part of the attack surface that the M8 security review covers (§14).

## Alternatives considered

- The plan records no rejected alternative.

## Validation

- **M3 exit criterion** (§14): Claude Code, through MCP, can add ligatures as a reviewable proposal.
- **Review rule:** MCP tools come only from the command registry (§6.9); `CLAUDE.md` forbids back doors for MCP.
- **Revisit** if users need access beyond localhost, or if "trusted automation" proves too broad.
