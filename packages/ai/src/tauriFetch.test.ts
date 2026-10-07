import { beforeEach, describe, expect, it, vi } from "vitest";
import { createFakeProxy, sseHead } from "./fakeProxy";

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

const { tauriFetch } = await import("./tauriFetch");

let proxy: ReturnType<typeof createFakeProxy>;

beforeEach(() => {
  proxy = createFakeProxy();
  ipc.aiFetch.mockReset().mockImplementation(proxy.aiFetch);
  ipc.aiAbort.mockReset().mockImplementation(proxy.aiAbort);
});

const URL_MESSAGES = "https://openrouter.ai/api/v1/messages?beta=true";

function bodyOf(response: Response): ReadableStream<Uint8Array> {
  if (response.body === null) throw new Error("no body");
  return response.body;
}

describe("tauriFetch", () => {
  it("forwards method, URL, headers and body with a fresh request ID", async () => {
    proxy.reply(sseHead, { kind: "end" });

    await tauriFetch(URL_MESSAGES, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        "anthropic-version": "2023-06-01",
      },
      body: '{"model":"m"}',
    });

    const [first] = proxy.calls;
    expect(first.request).toMatchObject({
      method: "POST",
      url: URL_MESSAGES,
      body: '{"model":"m"}',
    });
    expect(first.request.headers).toEqual(
      expect.arrayContaining([
        ["content-type", "application/json"],
        ["anthropic-version", "2023-06-01"],
      ]),
    );
    expect(first.request.requestId).toMatch(/^[A-Za-z0-9_-]{1,64}$/);
    await tauriFetch(new URL(URL_MESSAGES)).catch(() => {});
    expect(proxy.calls[1].request.requestId).not.toBe(first.request.requestId);
    expect(proxy.calls[1].request).toMatchObject({ method: "GET", body: null });
  });

  it("resolves on the head, before the body has ended", async () => {
    proxy.reply(
      {
        kind: "head",
        status: 201,
        headers: [
          ["content-type", "text/event-stream"],
          ["request-id", "req_1"],
        ],
      },
      { kind: "chunk", text: "first " },
      "wait",
      { kind: "chunk", text: "second" },
      { kind: "end" },
    );

    const response = await tauriFetch(URL_MESSAGES, {
      method: "POST",
      body: "{}",
    });
    expect(response.status).toBe(201);
    expect(response.headers.get("request-id")).toBe("req_1");

    const reader = bodyOf(response).getReader();
    const decoder = new TextDecoder();
    const first = await reader.read();
    expect(decoder.decode(first.value)).toBe("first ");
    expect(proxy.calls[0].events.map((e) => e.kind)).toEqual(["head", "chunk"]);

    proxy.calls[0].release();
    const second = await reader.read();
    expect(decoder.decode(second.value)).toBe("second");
    expect((await reader.read()).done).toBe(true);
  });

  it("keeps the body open until End, even if the command resolves first", async () => {
    // Tauri delivers large channel messages in a later IPC round trip, so the command's
    // own result can overtake the last chunks.
    ipc.aiFetch.mockImplementationOnce(
      async (_request, channel: { onmessage: (e: unknown) => void }) => {
        channel.onmessage(sseHead);
        setTimeout(() => {
          channel.onmessage({ kind: "chunk", text: "late" });
          channel.onmessage({ kind: "end" });
        }, 10);
        return { status: "ok", data: null };
      },
    );
    const response = await tauriFetch(URL_MESSAGES);
    expect(await response.text()).toBe("late");
  });

  it("rejects when the proxy refuses the request before a head", async () => {
    proxy.reply({ kind: "error", message: "the destination is not allowed" });
    await expect(tauriFetch("https://evil.example/")).rejects.toThrow(
      "the destination is not allowed",
    );
  });

  it("errors the body when the response fails after the head", async () => {
    proxy.reply(
      sseHead,
      { kind: "chunk", text: "a" },
      { kind: "error", message: "reset" },
    );
    const response = await tauriFetch(URL_MESSAGES);
    await expect(response.text()).rejects.toThrow("reset");
  });

  it("rejects when the command itself fails", async () => {
    ipc.aiFetch.mockResolvedValueOnce({
      status: "error",
      error: "not allowed",
    });
    await expect(tauriFetch(URL_MESSAGES)).rejects.toThrow("not allowed");

    ipc.aiFetch.mockRejectedValueOnce("command ai_fetch not allowed");
    await expect(tauriFetch(URL_MESSAGES)).rejects.toThrow(
      "command ai_fetch not allowed",
    );
  });

  it("rejects when the proxy ends without a head", async () => {
    proxy.reply({ kind: "end" });
    await expect(tauriFetch(URL_MESSAGES)).rejects.toThrow(
      /without a response/,
    );
  });

  it("maps an abort before the head to ai_abort and an AbortError", async () => {
    proxy.reply("wait", sseHead, { kind: "end" });
    const controller = new AbortController();
    const pending = tauriFetch(URL_MESSAGES, { signal: controller.signal });
    await vi.waitFor(() => expect(proxy.calls).toHaveLength(1));

    controller.abort();

    await expect(pending).rejects.toMatchObject({ name: "AbortError" });
    expect(ipc.aiAbort).toHaveBeenCalledWith(proxy.calls[0].request.requestId);
  });

  it("maps an abort after the head to ai_abort and errors the body", async () => {
    proxy.reply(sseHead, { kind: "chunk", text: "a" }, "wait", { kind: "end" });
    const controller = new AbortController();
    const response = await tauriFetch(URL_MESSAGES, {
      signal: controller.signal,
    });
    const reader = bodyOf(response).getReader();
    await reader.read();

    controller.abort();

    await expect(reader.read()).rejects.toMatchObject({ name: "AbortError" });
    expect(ipc.aiAbort).toHaveBeenCalledWith(proxy.calls[0].request.requestId);
  });

  it("cancelling the body aborts the request", async () => {
    proxy.reply(sseHead, "wait", { kind: "end" });
    const response = await tauriFetch(URL_MESSAGES);
    await bodyOf(response).cancel();
    expect(ipc.aiAbort).toHaveBeenCalledWith(proxy.calls[0].request.requestId);
  });

  it("never calls the proxy for an already aborted signal", async () => {
    const controller = new AbortController();
    controller.abort();
    await expect(
      tauriFetch(URL_MESSAGES, { signal: controller.signal }),
    ).rejects.toMatchObject({ name: "AbortError" });
    expect(ipc.aiFetch).not.toHaveBeenCalled();
  });

  it("accepts only string bodies and URL inputs", async () => {
    await expect(
      tauriFetch(URL_MESSAGES, { method: "POST", body: new Uint8Array([1]) }),
    ).rejects.toThrow(TypeError);
    await expect(tauriFetch(new Request(URL_MESSAGES))).rejects.toThrow(
      TypeError,
    );
    expect(ipc.aiFetch).not.toHaveBeenCalled();
  });

  it("rejects with an AbortError when Rust reports the request aborted", async () => {
    proxy.reply({ kind: "aborted" });
    await expect(tauriFetch(URL_MESSAGES)).rejects.toMatchObject({
      name: "AbortError",
    });
  });

  it("refuses events that break the proxy protocol", async () => {
    proxy.reply({ kind: "chunk", text: "x" }, sseHead, { kind: "end" });
    await expect(tauriFetch(URL_MESSAGES)).rejects.toThrow(
      /before the response head/,
    );

    proxy.reply({ kind: "head", status: 101, headers: [] }, { kind: "end" });
    // `Response` takes only statuses 200-599.
    await expect(tauriFetch(URL_MESSAGES)).rejects.toBeInstanceOf(TypeError);

    proxy.reply(sseHead, sseHead, { kind: "end" });
    const response = await tauriFetch(URL_MESSAGES);
    await expect(response.text()).rejects.toThrow(/second response head/);

    proxy.reply({ kind: "nope" } as never, { kind: "end" });
    await expect(tauriFetch(URL_MESSAGES)).rejects.toThrow(
      /unknown AI proxy event/,
    );
  });

  it("gives a null-body status no body", async () => {
    proxy.reply({ kind: "head", status: 204, headers: [] }, { kind: "end" });
    const response = await tauriFetch(URL_MESSAGES);
    expect(response.status).toBe(204);
    expect(response.body).toBeNull();
  });
});
