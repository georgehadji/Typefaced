# ADR-0002: Rust engine owns document state; the UI is a replica
Status: Proposed · Date: 2026-09-29

Source: [implementation plan](../implementation-plan.md) §0, §2 (principle 1), §3.3, §3.4, §5.12, §7.2, §7.3, §8.1, §10.4, §15 (R3); [research](../research.md) §5; [M0 plan](../../plans/typefaced-m0-foundations-and-spikes.md) Step 8.

## Context

- Four clients use the document: the UI, the in-app AI agent, the MCP server and the CLI (§0). The AI, MCP and CLI need headless access (§3.4).
- Every client goes through the same command API; no client has a private back door (§2, principle 1).
- Editing must feel instant: a point drag at 60 fps, and a commit round-trip under 8 ms at the 95th percentile (§10.4).

## Decision

- **The Rust engine owns the document.** The UI keeps a read-only replica, fed by patch events from the engine (§0, §7.2).
- **Committed state is always the engine's.** The replica only applies patches (§7.2).
- **Transient state stays in the UI:** point positions during a drag, marquee rectangles, snapping guides. It is computed locally with the WASM kernel ([ADR-0013](0013-ui-stack-react-zustand-canvas2d-wasm.md)) and discarded on commit (§7.2).
- **A gesture ends in one committed command** (§7.3, §8.1).
- **Optimistic updates** only for cheap, predictable commands, such as toggling a flag. They are rolled back if the engine rejects them (§7.2).
- **IPC** (§3.3):
  - commands and events typed with tauri-specta;
  - binary payloads for outlines and images;
  - Tauri channels for streams: patches, job progress and AI egress streams.
- **Packed outline payloads** (§7.2): `Float64Array` coordinates, `Uint8Array` point-type flags, `Uint32Array` contour ends.
- **Large fonts** load glyph data lazily. The core renders thumbnails when a font exceeds about 5,000 glyphs (§7.2).

## Consequences

- There is one validation path, and heavy operations stay in Rust (§3.4).
- The AI agent, MCP clients and the CLI edit the same engine without the UI.
- Every commit pays an IPC round-trip. Local transient previews and the WASM kernel absorb this cost during a gesture (§3.4, R3).
- Patches are built from the keys a reducer touched, so their cost scales with the change, not with the font (§5.12).
- UI code keeps two kinds of state apart: the committed replica and local transient state.

## Alternatives considered

- **A TypeScript document model with Rust file I/O** (Fontra's approach, recommended in research §5). Rejected: the AI, MCP and CLI need headless access, and owning the state in Rust keeps one validation path and the heavy operations in Rust (§3.4).

## Validation

**Validated by Step 8 — Spike 3: IPC latency and WASM geometry kernel.** Step 8 measures a commit round-trip (1,000 calls, each carrying a 5,000-point glyph edit, once as JSON and once as binary), a patch stream (60 patches per second for 10 s over a channel) and a 10 s synthetic drag.

Pass criteria (Step 8 exit criteria):
- With the better payload format, the commit round-trip p95 is under 8 ms.
- Drag frame-time p95 is under 16 ms.
- A hit-test across all contours of the 5,000-point glyph takes under 1 ms.
- The numbers and the chosen payload format are recorded.
- If a budget fails, the report names the mitigation and the ADR is amended. It is never silently accepted.

Step 8 then sets this ADR to Accepted, or amends it, for example "binary payloads are mandatory" or "move drag state further into the UI" (the R3 fallback).

## Amendment (Spike 3, 2026-10-03)

Report: [spike-3-ipc.md](../spikes/spike-3-ipc.md). Measured on one Windows laptop (i7-9750H, WebView2, release build of the Rust side).

- **Binary payloads are mandatory for outline data.** Raw request bodies and `tauri::ipc::Response` bytes in the packed layout. At 5,000 points the commit round trip has p95 22.5 to 25.0 ms in binary against 62.1 to 69.5 ms in JSON. Up to about 100 values JSON and binary are equal within the noise, so small commands may stay JSON.
- **The 8 ms commit budget failed.** An empty `invoke` already takes about 7 ms at p50 and 10 ms at p95 here, so no payload format can meet a p95 under 8 ms on this machine. A 1-point commit has p95 9.4 to 11.8 ms.
- **Commits are asynchronous and carry deltas.** The UI never blocks on a commit; it keeps its transient state until the patch arrives. Commits and patches carry the touched points, not whole outlines (100 points: p95 10.4 to 11.7 ms; whole 5,000-point glyph: 22 to 25 ms). Patches over a `Channel` are cheap when small (1-point patches arrive in about 1.5 ms) and miss a frame 8% to 24% of the time when they carry a whole glyph.
- **Hit-test budget met** (see ADR-0013).
- **Status stays Proposed.** It becomes Accepted when the manual run in the spike report's follow-ups shows the commit round trip within budget on idle hardware, or when the budget is re-stated, for example as "engine reducer and patch under 8 ms, the UI never waits".
