// `tauriFetch`: a `fetch` for the Anthropic SDK that sends each request to the Rust egress
// proxy (`ai_fetch`, ADR-0009) instead of the network. Rust checks the request, adds the
// API key and streams the response back over a channel: the head first, then text
// chunks, then one end, error or aborted marker. The returned promise resolves on the
// head; the body is a stream fed by the later chunks. Aborting the signal, or cancelling
// the body, calls `ai_abort`.
import { Channel } from "@tauri-apps/api/core";
import { commands, type ProxyEvent } from "@typefaced/bindings";

/** Statuses whose `Response` must not have a body (the constructor throws otherwise). */
const NULL_BODY_STATUSES = new Set([204, 205, 304]);

function abortError(): DOMException {
  return new DOMException("The request was aborted.", "AbortError");
}

function describe(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export async function tauriFetch(
  input: string | URL | Request,
  init: RequestInit = {},
): Promise<Response> {
  if (typeof input !== "string" && !(input instanceof URL)) {
    throw new TypeError("tauriFetch takes a URL, not a Request");
  }
  if (init.body != null && typeof init.body !== "string") {
    throw new TypeError("tauriFetch sends only string bodies");
  }
  const signal = init.signal ?? undefined;
  if (signal?.aborted) {
    throw abortError();
  }

  const requestId = crypto.randomUUID();
  const encoder = new TextEncoder();
  let body: ReadableStreamDefaultController<Uint8Array> | undefined;
  let settled = false;
  let hasHead = false;
  let resolveHead: (response: Response) => void = () => {};
  let rejectHead: (error: unknown) => void = () => {};
  const head = new Promise<Response>((resolve, reject) => {
    resolveHead = resolve;
    rejectHead = reject;
  });

  // Ends the request once: before the head the `fetch` promise rejects, after it the body.
  const fail = (error: unknown) => {
    if (settled) return;
    settled = true;
    signal?.removeEventListener("abort", abort);
    rejectHead(error);
    body?.error(error);
  };
  const finish = () => {
    if (settled) return;
    settled = true;
    signal?.removeEventListener("abort", abort);
    rejectHead(new TypeError("the AI proxy ended without a response"));
    body?.close();
  };
  const abort = () => {
    if (settled) return;
    // If the abort command itself fails there is nothing more to do from here.
    commands.aiAbort(requestId).catch(() => {});
    fail(abortError());
  };
  signal?.addEventListener("abort", abort, { once: true });

  const onEvent = (event: ProxyEvent) => {
    if (settled) return;
    switch (event.kind) {
      case "head": {
        if (hasHead) {
          fail(new TypeError("the AI proxy sent a second response head"));
          return;
        }
        hasHead = true;
        const stream = NULL_BODY_STATUSES.has(event.status)
          ? null
          : new ReadableStream<Uint8Array>({
              start: (controller) => {
                body = controller;
              },
              cancel: abort,
            });
        let response: Response;
        try {
          response = new Response(stream, {
            status: event.status,
            headers: event.headers,
          });
        } catch (error) {
          // A status outside 200-599 or a header value `Headers` refuses.
          fail(new TypeError(describe(error)));
          return;
        }
        resolveHead(response);
        // A null-body response is complete once its head is out.
        if (stream === null) finish();
        return;
      }
      case "chunk":
        if (!hasHead) {
          fail(
            new TypeError("the AI proxy sent data before the response head"),
          );
          return;
        }
        // ponytail: no backpressure (Rust cannot be paused); the SDK reads as it goes.
        body?.enqueue(encoder.encode(event.text));
        return;
      case "end":
        finish();
        return;
      case "error":
        fail(new TypeError(event.message));
        return;
      case "aborted":
        fail(abortError());
        return;
      default: {
        const unknown: never = event;
        fail(
          new TypeError(`unknown AI proxy event: ${JSON.stringify(unknown)}`),
        );
      }
    }
  };

  const channel = new Channel<ProxyEvent>();
  channel.onmessage = onEvent;
  const request = {
    requestId,
    method: init.method ?? "GET",
    url: String(input),
    headers: [...new Headers(init.headers).entries()],
    body: init.body ?? null,
  };
  // Rust always ends the channel with one `end`, `error` or `aborted` event, and the
  // command's own result can arrive before the last (large) chunks, so a successful
  // result settles nothing; only a failed command does.
  commands.aiFetch(request, channel).then(
    (result) => {
      if (result.status === "error") fail(new TypeError(result.error));
    },
    (error: unknown) => fail(new TypeError(describe(error))),
  );
  return head;
}
