# ADR-0011: Model configuration: app-side routes per task (OpenRouter); changes gated by evals
Status: Accepted · Date: 2026-09-29 · Amended: 2026-10-05 (OpenRouter routing)

Source: [implementation plan](../implementation-plan.md) §6.4, §6.5, §6.8, §13.1, §14 (M3), §15 (R13).

## Context

- The Claude API changes over time (R13).
- AI quality must not regress when a prompt, a model or an effort setting changes (§6.8).

## Decision

- **Models are app-side routes**, one per task, each a model plus up to 3 OpenRouter fallbacks (amended 2026-10-05; see below). This replaces "`claude-opus-5` for every playbook".
- **Settings live in one place, not spread through code.** In M0 that is the `ROUTES` table in `packages/ai/src/agent.ts`; a versioned config file (`assets/ai/config.toml`) remains the target when playbooks arrive (M3). They are re-verified against OpenRouter's API reference and with the `claude-api` skill before implementation (§6.4).
- **Settings as of 2026-09-29** (the full table is §6.4):
  - adaptive thinking;
  - effort `low` for metadata and quick answers, `high` by default, `xhigh` for long glyph-design runs, tuned per playbook with evals;
  - streaming for every agent call; every tool input is validated before it runs; `stop_reason` (`max_tokens`, `refusal`) is always checked before tools execute (`eager_input_streaming` is not sent through OpenRouter);
  - OpenRouter `fallbacks` for outages, rate limits and refusals, plus a clear UI message when a request is declined;
  - structured outputs for extraction tasks, and `strict: true` on tool schemas;
  - a fixed prompt-caching order: sorted tools → versioned system prompt → font-context digest → conversation.
- **Playbooks are data** (§6.5): each has a versioned `system.md`, a tool allowlist, a config (effort, budget, iteration cap) and success checks.
- **Any other model is a product decision.** A different model per route, or a newer model, is adopted only after the eval suite shows quality holds (§6.4).
- **No prompt, model or effort change may regress an eval suite** (§6.8).

## Amendment (2026-10-05): routes with OpenRouter fallbacks

User decision: "app picks + fallbacks". The app names the model for each task; OpenRouter's `fallbacks` (`[{model}]`, at most 3, tried in order when the model fails or refuses) cover outages and rate limits. No auto router (`openrouter/auto` is refused by the egress policy), no configuration system, no user picker.

| Task | Model | Fallbacks | Effort |
|---|---|---|---|
| `chat` (cheap, fast answers) | `anthropic/claude-sonnet-5.5` | `anthropic/claude-sonnet-5`, `anthropic/claude-sonnet-4.6` | `low` |
| `agent` (tool use; the Spike 4 loop) | `anthropic/claude-opus-5.5` | `anthropic/claude-opus-5`, `anthropic/claude-sonnet-5.5` | `medium` |

- Every ID was read from OpenRouter's public model list (`GET https://openrouter.ai/api/v1/models`, 2026-10-05); all of them list `tools` and `reasoning_effort` among their supported parameters.
- Claude models come first in both routes, since tool use is most reliable on them; the fallbacks are Claude models too.
- Adaptive thinking is sent on both routes; effort is per route.
- A route change is still a product decision gated by the eval suite (§6.8) once it exists.

## Consequences

- Changing the model is a one-line change to the route table plus an eval run.
- The eval suites become required infrastructure (§6.8). CI replays recorded responses, with no network and deterministic results; nightly runs call the live API under a budget cap.
- The UI shows the running cost per task and enforces per-task and monthly budgets (§6.4). Estimated at list prices of $5 / $25 per million input / output tokens: small tasks cost cents; a full "font from a description" run costs roughly $1–5. These estimates are measured in M3 (§6.4).
- The model names and beta features above are dated. They must be checked again before M3 builds the AI platform (§6.4, R13).

## Alternatives considered

- **Choosing per-route models up front**, for example `claude-fable-5-1` for the longest glyph-design runs or `claude-haiku-4-5` for per-cell handwriting labels. Not adopted by default: a route gets its own model only after the eval suite shows quality holds (§6.4).

## Validation

- **Eval gate** (§6.8): every prompt, model or effort change runs the eval suites and must not regress any of them. Eval harness v0 arrives in M3 (§14).
- **Re-verification:** the settings are checked with the `claude-api` skill before implementation (§6.4) and each quarter (R13).
- **Revisit** when a newer model or API feature becomes available, when an eval suite regresses, or when the costs measured in M3 differ widely from the estimates.
