import Anthropic from "@anthropic-ai/sdk";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  createFakeProxy,
  OPENROUTER_DONE,
  OPENROUTER_KEEPALIVE,
  sseHead,
  streamed,
  textTurn,
  toolTurn,
} from "./fakeProxy";

const ipc = vi.hoisted(() => ({
  aiFetch: vi.fn(),
  aiAbort: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  Channel: class {
    onmessage: (event: unknown) => void = () => {};
  },
}));
vi.mock("@typefaced/bindings", () => ({ commands: ipc }));

const { createClaudeClient, ROUTES, runAgent, KEY_PLACEHOLDER } = await import(
  "./agent"
);
const { createFontStore, FIXTURE_FONT, fontTools } = await import(
  "./fontTools"
);

let proxy: ReturnType<typeof createFakeProxy>;

beforeEach(() => {
  proxy = createFakeProxy();
  ipc.aiFetch.mockReset().mockImplementation(proxy.aiFetch);
  ipc.aiAbort.mockReset().mockImplementation(proxy.aiAbort);
});

/** Top-level body fields tf-ai-host lets through (crates/tf-ai-host/src/body.rs). */
const RUST_BODY_FIELDS = [
  "model",
  "max_tokens",
  "messages",
  "system",
  "tools",
  "thinking",
  "output_config",
  "stream",
  "fallbacks",
];
/** A model ID tf-ai-host lets through (`model_allowed` in body.rs). */
const RUST_MODEL_ID = /^anthropic\/[a-z0-9][a-z0-9._-]*$/;
const RUST_MAX_FALLBACKS = 3;

function setup(approve = vi.fn().mockResolvedValue(true)) {
  const store = createFontStore(FIXTURE_FONT);
  const onText = vi.fn();
  const run = (prompt: string, signal?: AbortSignal) =>
    runAgent(createClaudeClient(), prompt, {
      tools: fontTools(store, approve),
      onText,
      signal,
    });
  return { store, approve, onText, run };
}

function body(call: number) {
  return JSON.parse(proxy.calls[call].request.body ?? "null");
}

function header(call: number, name: string) {
  return proxy.calls[call].request.headers.find(([n]) => n === name)?.[1];
}

/** Request `call` carries only what tf-ai-host lets through. */
function expectAllowedByRust(call: number) {
  const sent = body(call);
  expect(Object.keys(sent).every((k) => RUST_BODY_FIELDS.includes(k))).toBe(
    true,
  );
  expect(sent.model).toMatch(RUST_MODEL_ID);
  expect(sent.fallbacks.length).toBeLessThanOrEqual(RUST_MAX_FALLBACKS);
  for (const fallback of sent.fallbacks) {
    expect(Object.keys(fallback)).toEqual(["model"]);
    expect(fallback.model).toMatch(RUST_MODEL_ID);
  }
  // tf-ai-host refuses every anthropic-beta header.
  expect(header(call, "anthropic-beta")).toBeUndefined();
}

/** The tool_result blocks of the last user message of request `call`. */
function toolResults(call: number) {
  const messages = body(call).messages;
  return messages.at(-1).content;
}

describe("runAgent", () => {
  it("sends the plan's request settings, and only what the egress policy allows", async () => {
    proxy.reply(...streamed(textTurn("Hello")));
    const { run, onText } = setup();

    const outcome = await run("Hi");

    expect(outcome).toEqual({ kind: "done", text: "Hello" });
    expect(onText).toHaveBeenCalledWith("Hello");
    const request = proxy.calls[0].request;
    expect(request.method).toBe("POST");
    expect(request.url).toBe("https://openrouter.ai/api/v1/messages?beta=true");
    const sent = body(0);
    expect(sent).toMatchObject({
      model: "anthropic/claude-opus-5.5",
      stream: true,
      thinking: { type: "adaptive" },
      output_config: { effort: "medium" },
      fallbacks: [
        { model: "anthropic/claude-opus-5" },
        { model: "anthropic/claude-sonnet-5.5" },
      ],
      messages: [{ role: "user", content: "Hi" }],
    });
    expectAllowedByRust(0);
    // Only a placeholder leaves the webview; Rust drops it and adds the real key.
    expect(header(0, "x-api-key")).toBe(KEY_PLACEHOLDER);
    expect(header(0, "authorization")).toBeUndefined();
    for (const tool of sent.tools) {
      expect(tool).not.toHaveProperty("eager_input_streaming");
    }
  });

  it("routes each task to its model and fallbacks", async () => {
    proxy.reply(...streamed(textTurn("Hi")));
    await runAgent(createClaudeClient(), "Hi", {
      task: "chat",
      tools: [],
      onText: () => {},
    });
    expect(body(0)).toMatchObject({
      model: ROUTES.chat.model,
      fallbacks: ROUTES.chat.fallbacks.map((model) => ({ model })),
      output_config: { effort: ROUTES.chat.effort },
    });
    expectAllowedByRust(0);
    for (const route of Object.values(ROUTES)) {
      // Claude models first: tool use is most reliable on them.
      expect(route.model).toMatch(/^anthropic\//);
      expect(route.fallbacks).not.toContain(route.model);
    }
  });

  it("reads OpenRouter's stream with keep-alive comments and [DONE]", async () => {
    const [first, ...rest] = textTurn("Hello").split("\n\n");
    // Comments between and inside network chunks, and the end marker on its own.
    const body = [
      OPENROUTER_KEEPALIVE,
      `${first}\n\n`,
      ": OPENROUTER",
      " PROCESSING\n\n",
      rest.join("\n\n"),
      OPENROUTER_DONE,
    ];
    proxy.reply(
      sseHead,
      ...body.map((text) => ({ kind: "chunk" as const, text })),
      { kind: "end" },
    );
    const { run, onText } = setup();

    expect(await run("Hi")).toEqual({ kind: "done", text: "Hello" });
    expect(onText).toHaveBeenCalledWith("Hello");
  });

  it("runs a tool call and sends its result back", async () => {
    proxy.reply(...streamed(toolTurn("toolu_1", "get_font_summary", "{}")));
    proxy.reply(...streamed(textTurn("The family is Typefaced Test.")));
    const { run } = setup();

    const outcome = await run("What is the family name?");

    expect(outcome).toEqual({
      kind: "done",
      text: "The family is Typefaced Test.",
    });
    const [result] = toolResults(1);
    expect(result).toMatchObject({
      type: "tool_result",
      tool_use_id: "toolu_1",
    });
    // The follow-up request, with the tool result, must pass the policy too.
    expectAllowedByRust(0);
    expectAllowedByRust(1);
    expect(result.is_error).toBeUndefined();
    expect(JSON.parse(result.content)).toEqual(FIXTURE_FONT);
  });

  it("never runs a tool whose input fails validation", async () => {
    proxy.reply(
      ...streamed(toolTurn("toolu_2", "set_family_name", '{"family_name":5}')),
    );
    proxy.reply(...streamed(textTurn("Sorry.")));
    const { run, store, approve } = setup();

    await run("Rename it");

    expect(approve).not.toHaveBeenCalled();
    expect(store.get()).toEqual(FIXTURE_FONT);
    const [result] = toolResults(1);
    expect(result).toMatchObject({ tool_use_id: "toolu_2", is_error: true });
    expect(result.content).toContain("INVALID_JSON");
  });

  it("returns a declined result when the user says no", async () => {
    proxy.reply(
      ...streamed(
        toolTurn("toolu_3", "set_family_name", '{"family_name":"Test Sans"}'),
      ),
    );
    proxy.reply(...streamed(textTurn("OK, left it.")));
    const { run, store, approve } = setup(vi.fn().mockResolvedValue(false));

    await run("Rename it to Test Sans");

    expect(approve).toHaveBeenCalledTimes(1);
    expect(store.get()).toEqual(FIXTURE_FONT);
    expect(toolResults(1)[0].content).toMatch(/declined/);
  });

  it("changes the font when the user approves", async () => {
    proxy.reply(
      ...streamed(
        toolTurn("toolu_4", "set_family_name", '{"family_name":"Test Sans"}'),
      ),
    );
    proxy.reply(...streamed(textTurn("Renamed.")));
    const { run, store } = setup();

    const outcome = await run("Rename it to Test Sans");

    expect(outcome).toEqual({ kind: "done", text: "Renamed." });
    expect(store.get().familyName).toBe("Test Sans");
    expect(toolResults(1)[0].is_error).toBeUndefined();
  });

  it("stops on a refusal without running that turn's tools", async () => {
    proxy.reply(
      ...streamed(
        toolTurn(
          "toolu_5",
          "set_family_name",
          '{"family_name":"X"}',
          "refusal",
        ),
      ),
    );
    const { run, approve } = setup();

    expect(await run("Rename")).toEqual({ kind: "refused" });
    expect(approve).not.toHaveBeenCalled();
    expect(proxy.calls).toHaveLength(1);
  });

  it.each(["max_tokens", "model_context_window_exceeded"])(
    "does not run a tool call that ended with %s",
    async (stopReason) => {
      proxy.reply(
        ...streamed(
          toolTurn(
            "toolu_6",
            "set_family_name",
            '{"family_name":"Te',
            stopReason,
          ),
        ),
      );
      const { run, approve, store } = setup();

      expect(await run("Rename")).toEqual({ kind: "truncated", stopReason });
      expect(approve).not.toHaveBeenCalled();
      expect(store.get()).toEqual(FIXTURE_FONT);
      expect(proxy.calls).toHaveLength(1);
    },
  );

  it("reports the iteration limit without running the last turn's tools", async () => {
    for (let i = 0; i < 8; i += 1) {
      proxy.reply(
        ...streamed(
          toolTurn(`toolu_l${i}`, "set_family_name", '{"family_name":"Loop"}'),
        ),
      );
    }
    const { run, approve } = setup();

    expect(await run("Loop")).toEqual({ kind: "iteration_limit" });
    expect(proxy.calls).toHaveLength(8);
    // The 8th turn's change would never reach the model, so it is not asked for.
    expect(approve).toHaveBeenCalledTimes(7);
  });

  it("aborts the request in Rust when the signal fires", async () => {
    proxy.reply(
      sseHead,
      { kind: "chunk", text: textTurn("partial").slice(0, 40) },
      "wait",
    );
    const { run } = setup();
    const controller = new AbortController();

    const pending = run("Hi", controller.signal);
    await vi.waitFor(() => expect(proxy.calls[0]?.events.length).toBe(2));
    controller.abort();

    await expect(pending).rejects.toBeInstanceOf(Anthropic.APIUserAbortError);
    expect(ipc.aiAbort).toHaveBeenCalledWith(proxy.calls[0].request.requestId);
  });
});
