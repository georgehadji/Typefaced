// Test support: a fake of the Rust egress proxy behind `ai_fetch` / `ai_abort`. Each
// call takes the next scripted reply; a reply is the list of channel events Rust would
// send, delivered one per macrotask so the caller can observe streaming. A reply can
// hold at a `"wait"` step until the test (or `ai_abort`) releases it.
import type { ProxyEvent, ProxyRequest } from "@typefaced/bindings";

export type ReplyStep = ProxyEvent | "wait";

export interface FakeCall {
  request: ProxyRequest;
  events: ProxyEvent[];
  release: () => void;
}

interface Channel {
  onmessage: (event: ProxyEvent) => void;
}

const tick = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

export function createFakeProxy() {
  const replies: ReplyStep[][] = [];
  const calls: FakeCall[] = [];
  const aborted = new Set<string>();

  async function aiFetch(request: ProxyRequest, channel: Channel) {
    // A release may come before the reply reaches its wait step; it is counted, not lost.
    let releases = 0;
    let waits = 0;
    let wake = () => {};
    const call: FakeCall = {
      request,
      events: [],
      release: () => {
        releases++;
        wake();
      },
    };
    calls.push(call);
    const reply = replies.shift() ?? [
      { kind: "error", message: "no scripted reply" },
    ];
    for (const step of reply) {
      await tick();
      if (aborted.has(request.requestId)) {
        break;
      }
      if (step === "wait") {
        waits++;
        while (releases < waits && !aborted.has(request.requestId)) {
          await new Promise<void>((resolve) => {
            wake = resolve;
          });
        }
        continue;
      }
      call.events.push(step);
      channel.onmessage(step);
    }
    if (aborted.has(request.requestId)) {
      call.events.push({ kind: "aborted" });
      channel.onmessage({ kind: "aborted" });
    }
    return { status: "ok" as const, data: null };
  }

  function aiAbort(requestId: string) {
    aborted.add(requestId);
    for (const call of calls) {
      if (call.request.requestId === requestId) {
        call.release();
      }
    }
    return Promise.resolve(true);
  }

  return {
    calls,
    aiFetch,
    aiAbort,
    /** Queues the events of the next `ai_fetch` reply. */
    reply(...steps: ReplyStep[]) {
      replies.push(steps);
    },
  };
}

/** A 200 response head for an SSE body. */
export const sseHead: ProxyEvent = {
  kind: "head",
  status: 200,
  headers: [["content-type", "text/event-stream"]],
};

/** One SSE frame per event, in the Messages API streaming format. */
export function sse(events: { type: string }[]): string {
  return events
    .map((event) => `event: ${event.type}\ndata: ${JSON.stringify(event)}\n\n`)
    .join("");
}

const usage = { input_tokens: 10, output_tokens: 1 };

function messageStart(id: string) {
  return {
    type: "message_start",
    message: {
      id,
      type: "message",
      role: "assistant",
      model: "anthropic/claude-opus-5.5",
      content: [],
      stop_reason: null,
      stop_sequence: null,
      usage,
    },
  };
}

function messageEnd(stopReason: string) {
  return [
    {
      type: "message_delta",
      delta: { stop_reason: stopReason, stop_sequence: null },
      usage: { output_tokens: 5 },
    },
    { type: "message_stop" },
  ];
}

/** A streamed assistant turn that answers with `text`. */
export function textTurn(text: string, stopReason = "end_turn"): string {
  return sse([
    messageStart("msg_text"),
    {
      type: "content_block_start",
      index: 0,
      content_block: { type: "text", text: "" },
    } as { type: string },
    {
      type: "content_block_delta",
      index: 0,
      delta: { type: "text_delta", text },
    } as { type: string },
    { type: "content_block_stop", index: 0 } as { type: string },
    ...messageEnd(stopReason),
  ]);
}

/** A streamed assistant turn that calls tool `name`; `inputJson` is sent as given. */
export function toolTurn(
  id: string,
  name: string,
  inputJson: string,
  stopReason = "tool_use",
): string {
  return sse([
    messageStart(`msg_${id}`),
    {
      type: "content_block_start",
      index: 0,
      content_block: { type: "tool_use", id, name, input: {} },
    } as { type: string },
    {
      type: "content_block_delta",
      index: 0,
      delta: { type: "input_json_delta", partial_json: inputJson },
    } as { type: string },
    { type: "content_block_stop", index: 0 } as { type: string },
    ...messageEnd(stopReason),
  ]);
}

/**
 * OpenRouter's keep-alive comment and end marker, which wrap its Messages API streams
 * (https://openrouter.ai/docs/api/reference/streaming).
 */
export const OPENROUTER_KEEPALIVE = ": OPENROUTER PROCESSING\n\n";
export const OPENROUTER_DONE = "data: [DONE]\n\n";

/** The scripted reply for a whole SSE body as OpenRouter sends it, in one chunk. */
export function streamed(body: string): ReplyStep[] {
  const text = `${OPENROUTER_KEEPALIVE}${body}${OPENROUTER_DONE}`;
  return [sseHead, { kind: "chunk", text }, { kind: "end" }];
}
