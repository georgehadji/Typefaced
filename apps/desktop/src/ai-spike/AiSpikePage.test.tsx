import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import type { AgentOptions } from "@typefaced/ai";
import { commands } from "@typefaced/bindings";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import AiSpikePage from "./AiSpikePage";

const agent = vi.hoisted(() => ({ runAgent: vi.fn() }));

vi.mock("@typefaced/bindings", () => ({
  commands: {
    aiHasKey: vi.fn(),
    aiSetKey: vi.fn(),
    aiDeleteKey: vi.fn(),
  },
}));
vi.mock("@typefaced/ai", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@typefaced/ai")>()),
  createClaudeClient: vi.fn(() => ({})),
  runAgent: agent.runAgent,
}));

const ok = <T,>(data: T) => ({ status: "ok" as const, data });

/** A `runAgent` that calls `set_family_name`, then streams `text`. */
function renamingAgent(text: string) {
  return async (_client: unknown, _prompt: string, options: AgentOptions) => {
    const result = await options.tools[1].run({ family_name: "Test Sans" });
    options.onText(text);
    return { kind: "done", text: String(result) };
  };
}

function runPrompt(prompt = "Rename it to Test Sans") {
  fireEvent.change(screen.getByLabelText("Prompt"), {
    target: { value: prompt },
  });
  fireEvent.click(screen.getByRole("button", { name: "Run" }));
}

describe("AiSpikePage", () => {
  beforeEach(() => {
    vi.mocked(commands.aiHasKey).mockResolvedValue(ok(false));
    vi.mocked(commands.aiSetKey).mockResolvedValue(ok(null));
    vi.mocked(commands.aiDeleteKey).mockResolvedValue(ok(null));
  });

  afterEach(() => {
    cleanup();
    vi.resetAllMocks();
  });

  it("shows whether a key is stored", async () => {
    vi.mocked(commands.aiHasKey).mockResolvedValue(ok(true));
    render(<AiSpikePage />);
    expect(await screen.findByText("Key stored: yes")).toBeTruthy();
  });

  it("sends the key to Rust once and clears the field", async () => {
    render(<AiSpikePage />);
    await screen.findByText("Key stored: no");
    const field = screen.getByLabelText(
      "OpenRouter API key",
    ) as HTMLInputElement;
    expect(field.type).toBe("password");
    vi.mocked(commands.aiHasKey).mockResolvedValue(ok(true));

    fireEvent.change(field, { target: { value: "test-key-value" } });
    fireEvent.click(screen.getByRole("button", { name: "Store key" }));

    expect(field.value).toBe("");
    await screen.findByText("Key stored: yes");
    expect(commands.aiSetKey).toHaveBeenCalledTimes(1);
    expect(commands.aiSetKey).toHaveBeenCalledWith("test-key-value");
  });

  it("ignores an empty key and reports a failure without the key", async () => {
    render(<AiSpikePage />);
    await screen.findByText("Key stored: no");
    fireEvent.click(screen.getByRole("button", { name: "Store key" }));
    expect(commands.aiSetKey).not.toHaveBeenCalled();

    vi.mocked(commands.aiSetKey).mockResolvedValue({
      status: "error",
      error: "the key must be 1 to 512 visible ASCII characters",
    });
    fireEvent.change(screen.getByLabelText("OpenRouter API key"), {
      target: { value: "bad key" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Store key" }));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("1 to 512");
    expect(alert.textContent).not.toContain("bad key");
  });

  it("deletes the key", async () => {
    vi.mocked(commands.aiHasKey).mockResolvedValue(ok(true));
    render(<AiSpikePage />);
    await screen.findByText("Key stored: yes");
    vi.mocked(commands.aiHasKey).mockResolvedValue(ok(false));
    fireEvent.click(screen.getByRole("button", { name: "Delete key" }));
    await screen.findByText("Key stored: no");
    expect(commands.aiDeleteKey).toHaveBeenCalledTimes(1);
  });

  it("asks for approval, then shows the change and the streamed answer", async () => {
    agent.runAgent.mockImplementation(renamingAgent("Renamed."));
    render(<AiSpikePage />);
    expect(screen.getByText("Family name: Typefaced Test")).toBeTruthy();

    runPrompt();
    const dialog = await screen.findByRole("dialog");
    expect(dialog.textContent).toContain('"Typefaced Test" to "Test Sans"');
    fireEvent.click(screen.getByRole("button", { name: "Approve" }));

    await screen.findByText("Family name: Test Sans");
    await screen.findByText("Renamed.");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(agent.runAgent.mock.calls[0][1]).toBe("Rename it to Test Sans");
  });

  it("leaves the font unchanged when the user declines", async () => {
    agent.runAgent.mockImplementation(renamingAgent("Left it."));
    render(<AiSpikePage />);
    runPrompt();
    await screen.findByRole("dialog");
    fireEvent.click(screen.getByRole("button", { name: "Decline" }));
    await screen.findByText("Left it.");
    expect(screen.getByText("Family name: Typefaced Test")).toBeTruthy();
  });

  it("reports a refusal and a truncated tool call", async () => {
    agent.runAgent.mockResolvedValueOnce({ kind: "refused" });
    render(<AiSpikePage />);
    runPrompt();
    await screen.findByText(/declined to answer/);

    agent.runAgent.mockResolvedValueOnce({
      kind: "truncated",
      stopReason: "max_tokens",
    });
    runPrompt();
    await screen.findByText(/cut off \(stop reason: max_tokens\)/);
  });

  it("shows why a request failed, including the proxy's reason", async () => {
    agent.runAgent.mockRejectedValueOnce(
      Object.assign(new Error("Connection error."), {
        cause: new TypeError("anthropic-beta headers are not forwarded"),
      }),
    );
    render(<AiSpikePage />);
    runPrompt();
    await screen.findByText(
      "Failed: Connection error. (anthropic-beta headers are not forwarded)",
    );
  });

  it("disables Run while running and declines a second approval at once", async () => {
    let second: Promise<unknown> | undefined;
    agent.runAgent.mockImplementation(
      (_client: unknown, _prompt: string, options: AgentOptions) => {
        const first = Promise.resolve(
          options.tools[1].run({ family_name: "A" }),
        );
        second = Promise.resolve(options.tools[1].run({ family_name: "B" }));
        return first.then(() => ({ kind: "done", text: "" }));
      },
    );
    render(<AiSpikePage />);
    runPrompt();
    await screen.findByRole("dialog");
    const run = screen.getByRole("button", {
      name: "Run",
    }) as HTMLButtonElement;
    expect(run.disabled).toBe(true);
    expect(String(await second)).toContain("declined");

    fireEvent.click(screen.getByRole("button", { name: "Approve" }));
    await screen.findByText("Family name: A");
    await waitFor(() => expect(run.disabled).toBe(false));
  });

  it("stops a running request and declines a pending approval", async () => {
    let toolResult: Promise<unknown> | undefined;
    let signal: AbortSignal | undefined;
    agent.runAgent.mockImplementation(
      (_client: unknown, _prompt: string, options: AgentOptions) =>
        new Promise((_resolve, reject) => {
          signal = options.signal;
          toolResult = Promise.resolve(
            options.tools[1].run({ family_name: "X" }),
          );
          options.signal?.addEventListener("abort", () =>
            reject(new Error("Request was aborted.")),
          );
        }),
    );
    render(<AiSpikePage />);
    runPrompt();
    await screen.findByRole("dialog");

    fireEvent.click(screen.getByRole("button", { name: "Stop" }));

    await screen.findByText(/Request was aborted/);
    expect(String(await toolResult)).toContain("declined");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(screen.getByText("Family name: Typefaced Test")).toBeTruthy();

    // A tool call that arrives after Stop is declined without asking.
    const agentOptions = agent.runAgent.mock.calls[0][2] as AgentOptions;
    expect(signal?.aborted).toBe(true);
    expect(
      String(await agentOptions.tools[1].run({ family_name: "Y" })),
    ).toContain("declined");
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("stops the running request when the page goes away", async () => {
    let signal: AbortSignal | undefined;
    agent.runAgent.mockImplementation(
      (_client: unknown, _prompt: string, options: AgentOptions) => {
        signal = options.signal;
        return new Promise(() => {});
      },
    );
    const { unmount } = render(<AiSpikePage />);
    runPrompt();
    await waitFor(() => expect(signal).toBeDefined());
    unmount();
    expect(signal?.aborted).toBe(true);
  });

  it("shows an IPC failure of a key command", async () => {
    vi.mocked(commands.aiHasKey).mockRejectedValue(new Error("ipc down"));
    render(<AiSpikePage />);
    expect((await screen.findByRole("alert")).textContent).toBe("ipc down");
  });
});
