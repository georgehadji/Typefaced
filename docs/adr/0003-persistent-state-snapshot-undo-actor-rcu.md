# ADR-0003: Persistent immutable state; snapshot undo; actor + read-copy-update
Status: Accepted · Date: 2026-09-29

Source: [implementation plan](../implementation-plan.md) §2 (principles 3, 5), §3.1, §3.3, §3.4, §5.1, §5.12, §9, §10.4, §10.5, §14 (M1); [research](../research.md) §5; [M0 plan](../../plans/typefaced-m0-foundations-and-spikes.md) Step 3.1.

## Context

- The engine needs undo and redo, sandboxes for AI proposals, background jobs, autosave and crash recovery (§5.12).
- Many parts read the document at once (UI patches, queries, jobs, autosave), and readers must never block (§3.3, §5.12).
- Principle 3: document state is persistent and structurally shared; one actor writes; everyone else reads snapshots without locks (§2).

## Decision

- **Persistent, structurally shared state** (§5.1). Cloning a whole `Font` copies a handful of pointers. `Arc<Glyph>` and `Arc<Layer>` are shared across snapshots, and copy-on-write happens through `Arc::make_mut` inside reducers only. The collection behind the glyph table is decided by [ADR-0017](0017-persistent-collections.md).
- **Pure reducers** (§5.12): `(DocState, Command) -> Result<(DocState, Vec<DomainEvent>, TouchedSet)>`.
- **One actor per open document** is the only writer. Its mailbox serialises writes (§3.3, §5.12).
- **Read-copy-update.** The latest snapshot is published through `ArcSwap`. Readers get it without locks and without messaging the actor (§3.3).
- **Snapshot undo** (§5.12). History is a list of `Arc<DocState>` roots with labels. Continuous gestures such as nudges merge within a 500 ms window. A transaction turns several commands into one undo step.
- **Background jobs** compute on a snapshot and commit their results as ordinary commands carrying their base revision. The engine accepts them, re-bases them, or asks for a recompute (optimistic concurrency; §3.3, §5.12).
- **A command journal** per document (append-only) serves crash recovery and auditing. It is not the primary storage (§3.1, §5.12).

## Consequences

- Undo is correct by construction. The same mechanism gives AI sandboxes, background jobs and lock-free autosave (§3.4).
- There is no shared mutable document state and no lock around it, so the engine is deadlock-free by construction (§10.5).
- Autosave writes immutable snapshots from a background task without locking (§9).
- History holds many snapshots, so memory depends on structural sharing. Budget: a 1,000-glyph project with 200 undo steps stays under 500 MB (§10.4).
- Reducers must stay pure. File access, network and OS services stay at the edges, behind ports (§2, principle 5).

## Alternatives considered

- **An inverse-operation log**, for example change objects that each carry a rollback change (Fontra's design, research §5). Rejected: snapshot history is correct by construction, and the same mechanism also serves sandboxes, jobs and autosave (§3.4).

## Validation

- **Tests** (§5.12): reducer unit tests; property tests for `undo ∘ redo = id`, "fork + discard leaves the base untouched" and "merge(all) equals applying the sandbox's commands to the base when nothing conflicts"; journal replay and concurrency tests.
- **M1 exit criteria** (§14): undo and sandbox property tests green, and coverage of at least 90%. `cargo xtask coverage` gates domain and application crates at 90% (Step 3.1).
- **Budget:** the §10.4 memory budget runs in CI as a benchmark.
- **Revisit** if the memory budget fails, or if the [ADR-0017](0017-persistent-collections.md) spike finds no collection that meets its thresholds.
