// The Claude client for the webview and the Spike 4 agent loop. The client talks to
// OpenRouter's Anthropic-compatible Messages API; its `fetch` is `tauriFetch`, so every
// request goes through the Rust egress proxy, which drops the placeholder key and adds
// the real one as a bearer token (ADR-0009). The loop is the SDK's beta tool runner,
// streamed, with the request settings of implementation plan §6.4 and the model routes
// of ADR-0011.
import Anthropic from "@anthropic-ai/sdk";
import type { BetaRunnableTool } from "@anthropic-ai/sdk/lib/tools/BetaRunnableTool";
import { tauriFetch } from "./tauriFetch";

/** OpenRouter's Anthropic-compatible API; the SDK appends `/v1/messages`. */
export const OPENROUTER_BASE_URL = "https://openrouter.ai/api";

export type Task = "chat" | "agent";

export interface Route {
  /** OpenRouter model ID `anthropic/<model>`, no `:variant` (Rust refuses others). */
  model: string;
  /** Tried in order if the model fails or refuses; OpenRouter takes at most 3. */
  fallbacks: string[];
  effort: "low" | "medium" | "high";
}

/**
 * The app picks the model per task; OpenRouter's `fallbacks` cover outages, rate limits
 * and refusals. IDs are from OpenRouter's public model list (2026-10-05).
 */
export const ROUTES: Readonly<Record<Task, Route>> = {
  chat: {
    model: "anthropic/claude-sonnet-5.5",
    fallbacks: ["anthropic/claude-sonnet-5", "anthropic/claude-sonnet-4.6"],
    effort: "low",
  },
  agent: {
    model: "anthropic/claude-opus-5.5",
    fallbacks: ["anthropic/claude-opus-5", "anthropic/claude-sonnet-5.5"],
    effort: "medium",
  },
};
/**
 * Sent as `x-api-key` by the SDK, which requires some key. The Rust proxy drops every
 * incoming `x-api-key` and `authorization` and adds the stored key as a bearer token.
 * Must never look like a real key.
 */
export const KEY_PLACEHOLDER = "injected-by-rust";

const MAX_TOKENS = 64000;
/** Bounds the request → tool → request loop. */
const MAX_ITERATIONS = 8;

const SYSTEM_PROMPT =
  "You are the assistant of Typefaced, a font editor. Use the tools to read and change " +
  "the open font. Answer briefly.";

export function createClaudeClient(): Anthropic {
  return new Anthropic({
    apiKey: KEY_PLACEHOLDER,
    baseURL: OPENROUTER_BASE_URL,
    fetch: tauriFetch,
    // Never pick up ANTHROPIC_AUTH_TOKEN from the environment (tests, Node tools).
    authToken: null,
    // The SDK refuses to run in a browser by default because the key would be exposed;
    // here the webview holds only the placeholder.
    dangerouslyAllowBrowser: true,
  });
}

export type AgentOutcome =
  | { kind: "done"; text: string }
  /** The model declined (`stop_reason: "refusal"`), also after any fallback. */
  | { kind: "refused" }
  /**
   * The turn has a tool call but did not stop for it (`max_tokens`, context window …),
   * so its input may be cut off; it was not run.
   */
  | { kind: "truncated"; stopReason: string | null }
  /** The loop hit `max_iterations` while the model still wanted tools. */
  | { kind: "iteration_limit" };

export interface AgentOptions {
  /** Picks the model route; the agent loop defaults to `agent`. */
  task?: Task;
  tools: BetaRunnableTool[];
  /** Receives the answer text as it streams. */
  onText: (delta: string) => void;
  signal?: AbortSignal;
}

/** Runs one user prompt to completion; tools run between turns. */
export async function runAgent(
  client: Anthropic,
  prompt: string,
  { task = "agent", tools, onText, signal }: AgentOptions,
): Promise<AgentOutcome> {
  const route = ROUTES[task];
  const runner = client.beta.messages.toolRunner(
    {
      model: route.model,
      max_tokens: MAX_TOKENS,
      thinking: { type: "adaptive" },
      output_config: { effort: route.effort },
      // OpenRouter's shape (each entry only `model`), not Anthropic's `"default"`.
      fallbacks: route.fallbacks.map((model) => ({ model })),
      system: SYSTEM_PROMPT,
      tools,
      messages: [{ role: "user", content: prompt }],
      max_iterations: MAX_ITERATIONS,
      stream: true,
      // Tools run only after this loop has seen the turn's stop_reason (the runner's
      // default); running them eagerly would skip the refusal and truncation checks.
    },
    { signal },
  );
  let text = "";
  let turn = 0;
  for await (const stream of runner) {
    turn += 1;
    stream.on("text", (delta) => onText(delta));
    const message = await stream.finalMessage();
    // Checked before the runner would run this turn's tools: a refusal can cut a
    // tool_use off mid-input, and a truncated input can still pass validation.
    if (message.stop_reason === "refusal") {
      return { kind: "refused" };
    }
    const hasToolUse = message.content.some(
      (block) => block.type === "tool_use",
    );
    // Tool calls run only after a `tool_use` stop; any other stop may have cut one off.
    if (hasToolUse && message.stop_reason !== "tool_use") {
      return { kind: "truncated", stopReason: message.stop_reason };
    }
    // The runner would run the last allowed turn's tools but never send their results;
    // returning here stops it before an approved change happens outside the dialogue.
    if (hasToolUse && turn === MAX_ITERATIONS) {
      return { kind: "iteration_limit" };
    }
    text = message.content
      .flatMap((block) => (block.type === "text" ? [block.text] : []))
      .join("");
  }
  return { kind: "done", text };
}
