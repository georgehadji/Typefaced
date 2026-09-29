# ADR-0011: Model configuration: `claude-opus-5` by default; config-driven; changes gated by evals
Status: Accepted · Date: 2026-09-29

Source: [implementation plan](../implementation-plan.md) §6.4, §6.5, §6.8, §13.1, §14 (M3), §15 (R13).

## Context

- The Claude API changes over time (R13).
- AI quality must not regress when a prompt, a model or an effort setting changes (§6.8).

## Decision

- **The default model is `claude-opus-5`** for every playbook (§6.4).
- **Settings live in a versioned config file**, `assets/ai/config.toml`, not in code. They are re-verified with the `claude-api` skill before implementation (§6.4).
- **Settings as of 2026-09-29** (the full table is §6.4):
  - adaptive thinking;
  - effort `low` for metadata and quick answers, `high` by default, `xhigh` for long glyph-design runs, tuned per playbook with evals;
  - streaming for every agent call; client tools set `eager_input_streaming: true`, so every tool input is validated before it runs; `stop_reason` (`max_tokens`, `refusal`) is always checked before tools execute;
  - server-side fallbacks (beta) for refusals, plus a clear UI message when a request is declined;
  - structured outputs for extraction tasks, and `strict: true` on tool schemas;
  - a fixed prompt-caching order: sorted tools → versioned system prompt → font-context digest → conversation.
- **Playbooks are data** (§6.5): each has a versioned `system.md`, a tool allowlist, a config (effort, budget, iteration cap) and success checks.
- **Any other model is a product decision.** A different model per route, or a newer model, is adopted only after the eval suite shows quality holds (§6.4).
- **No prompt, model or effort change may regress an eval suite** (§6.8).

## Consequences

- Changing the model is a config change plus an eval run, not a code change.
- The eval suites become required infrastructure (§6.8). CI replays recorded responses, with no network and deterministic results; nightly runs call the live API under a budget cap.
- The UI shows the running cost per task and enforces per-task and monthly budgets (§6.4). Estimated at list prices of $5 / $25 per million input / output tokens: small tasks cost cents; a full "font from a description" run costs roughly $1–5. These estimates are measured in M3 (§6.4).
- The model names and beta features above are dated. They must be checked again before M3 builds the AI platform (§6.4, R13).

## Alternatives considered

- **Choosing per-route models up front**, for example `claude-fable-5-1` for the longest glyph-design runs or `claude-haiku-4-5` for per-cell handwriting labels. Not adopted by default: a route gets its own model only after the eval suite shows quality holds (§6.4).

## Validation

- **Eval gate** (§6.8): every prompt, model or effort change runs the eval suites and must not regress any of them. Eval harness v0 arrives in M3 (§14).
- **Re-verification:** the settings are checked with the `claude-api` skill before implementation (§6.4) and each quarter (R13).
- **Revisit** when a newer model or API feature becomes available, when an eval suite regresses, or when the costs measured in M3 differ widely from the estimates.
