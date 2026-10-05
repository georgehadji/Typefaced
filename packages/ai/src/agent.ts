// The Claude client for the webview and the Spike 4 agent loop. The client's `fetch` is
// `tauriFetch`, so every request goes through the Rust egress proxy, which drops the
// placeholder key and adds the real one (ADR-0009). The loop is the SDK's beta tool
// runner, streamed, with the request settings of implementation plan §6.4.
import Anthropic from "@anthropic-ai/sdk";
import type { BetaRunnableTool } from "@anthropic-ai/sdk/lib/tools/BetaRunnableTool";
import { tauriFetch } from "./tauriFetch";

/** The default model of the `claude-api` skill (the plan said `claude-opus-5`). */
export const MODEL = "claude-opus-5-5";
/** Beta header for `fallbacks: "default"`; tf-ai-host allows this value only. */
export const FALLBACK_BETA = "server-side-fallback-2026-07-01";
/**
 * Sent as `x-api-key` by the SDK, which requires some key. The Rust proxy drops every
 * incoming `x-api-key` and adds the stored one. Must never look like a real key.
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
    fetch: tauriFetch,
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
  tools: BetaRunnableTool[];
  /** Receives the answer text as it streams. */
  onText: (delta: string) => void;
  signal?: AbortSignal;
}

/** Runs one user prompt to completion; tools run between turns. */
export async function runAgent(
  client: Anthropic,
  prompt: string,
  { tools, onText, signal }: AgentOptions,
): Promise<AgentOutcome> {
  const runner = client.beta.messages.toolRunner(
    {
      model: MODEL,
      max_tokens: MAX_TOKENS,
      thinking: { type: "adaptive" },
      // Claude Opus 5.5 defaults to `medium`; set it explicitly.
      output_config: { effort: "medium" },
      fallbacks: "default",
      betas: [FALLBACK_BETA],
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
  let wantsTools = false;
  for await (const stream of runner) {
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
    wantsTools = hasToolUse;
    text = message.content
      .flatMap((block) => (block.type === "text" ? [block.text] : []))
      .join("");
  }
  // After the last allowed turn the runner runs its tools but sends no results.
  if (wantsTools) return { kind: "iteration_limit" };
  return { kind: "done", text };
}
