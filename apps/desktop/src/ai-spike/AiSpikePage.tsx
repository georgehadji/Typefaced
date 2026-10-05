import {
  type AgentOutcome,
  type ApprovalRequest,
  createClaudeClient,
  createFontStore,
  FIXTURE_FONT,
  fontTools,
  runAgent,
} from "@typefaced/ai";
import { commands } from "@typefaced/bindings";
import {
  type FormEvent,
  useEffect,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import "./ai-spike.css";

interface PendingApproval {
  request: ApprovalRequest;
  answer: (approved: boolean) => void;
}

function describeError(error: unknown): string {
  if (!(error instanceof Error)) return String(error);
  // The SDK reports a refused or failed proxy request as "Connection error." and keeps
  // the proxy's message as the cause.
  const { cause } = error as { cause?: unknown };
  return cause instanceof Error
    ? `${error.message} (${cause.message})`
    : error.message;
}

function describeOutcome(outcome: AgentOutcome): string {
  switch (outcome.kind) {
    case "done":
      return "Done.";
    case "refused":
      return "The model declined to answer (stop reason: refusal).";
    case "truncated":
      return `A tool call was cut off (stop reason: ${outcome.stopReason}); it was not run.`;
    case "iteration_limit":
      return "Stopped at the tool-call limit; the last tool results were not sent.";
  }
}

/**
 * Dev-only page for Spike 4 (M0 Step 9.2): store the Claude API key, then run a prompt
 * through the SDK, the Rust egress proxy and the two demo tools.
 */
export default function AiSpikePage() {
  const [client] = useState(createClaudeClient);
  const [store] = useState(() => createFontStore(FIXTURE_FONT));
  const font = useSyncExternalStore(store.subscribe, store.get);
  const keyField = useRef<HTMLInputElement>(null);
  const [hasKey, setHasKey] = useState<boolean>();
  const [keyError, setKeyError] = useState<string>();
  const [prompt, setPrompt] = useState("");
  const [output, setOutput] = useState("");
  const [status, setStatus] = useState("");
  const [approval, setApproval] = useState<PendingApproval>();
  const running = useRef<AbortController>(undefined);
  const [isRunning, setIsRunning] = useState(false);
  const pendingApproval = useRef(false);

  const refreshKey = async () => {
    const result = await commands.aiHasKey();
    if (result.status === "ok") setHasKey(result.data);
    else setKeyError(result.error);
  };

  /** Runs a key command; an IPC failure is shown like a command error. */
  const keyAction = async (action: () => Promise<void>) => {
    try {
      await action();
    } catch (error) {
      setKeyError(describeError(error));
    }
  };

  // biome-ignore lint/correctness/useExhaustiveDependencies: runs once on mount.
  useEffect(() => {
    void keyAction(refreshKey);
    // Leaving the page stops a running request (and declines its pending approval).
    return () => running.current?.abort();
  }, []);

  const storeKey = (event: FormEvent) => {
    event.preventDefault();
    const field = keyField.current;
    const key = field?.value ?? "";
    // The key is kept nowhere: the field is cleared before it is sent.
    if (field) field.value = "";
    if (key === "") return;
    setKeyError(undefined);
    void keyAction(async () => {
      const result = await commands.aiSetKey(key);
      if (result.status === "error") setKeyError(result.error);
      await refreshKey();
    });
  };

  const deleteKey = () =>
    keyAction(async () => {
      const result = await commands.aiDeleteKey();
      if (result.status === "error") setKeyError(result.error);
      await refreshKey();
    });

  /** Approval for one run: declined at once, or on Stop, once the run is aborted. */
  const approveFor = (signal: AbortSignal) => (request: ApprovalRequest) =>
    new Promise<boolean>((resolve) => {
      // The SDK runs a turn's tool calls at once; only one change is shown at a time.
      if (signal.aborted || pendingApproval.current) {
        resolve(false);
        return;
      }
      pendingApproval.current = true;
      const answer = (approved: boolean) => {
        signal.removeEventListener("abort", decline);
        pendingApproval.current = false;
        setApproval(undefined);
        resolve(approved);
      };
      const decline = () => answer(false);
      signal.addEventListener("abort", decline, { once: true });
      setApproval({ request, answer });
    });

  const run = async (event: FormEvent) => {
    event.preventDefault();
    const controller = new AbortController();
    running.current = controller;
    setIsRunning(true);
    setOutput("");
    setStatus("Running…");
    try {
      const outcome = await runAgent(client, prompt, {
        tools: fontTools(store, approveFor(controller.signal)),
        onText: (delta) => setOutput((text) => text + delta),
        signal: controller.signal,
      });
      setStatus(describeOutcome(outcome));
    } catch (error) {
      setStatus(`Failed: ${describeError(error)}`);
    } finally {
      if (running.current === controller) running.current = undefined;
      setIsRunning(false);
    }
  };

  return (
    <main className="ai-spike">
      <h1>AI spike</h1>
      <form onSubmit={storeKey}>
        <label>
          Claude API key{" "}
          <input ref={keyField} type="password" autoComplete="off" />
        </label>{" "}
        <button type="submit">Store key</button>{" "}
        <button type="button" onClick={deleteKey}>
          Delete key
        </button>
        <p>Key stored: {hasKey === undefined ? "…" : hasKey ? "yes" : "no"}</p>
        {keyError && <p role="alert">{keyError}</p>}
      </form>
      <p>Family name: {font.familyName}</p>
      <form onSubmit={run}>
        <label>
          Prompt
          <textarea
            value={prompt}
            onChange={(event) => setPrompt(event.target.value)}
            rows={3}
          />
        </label>
        <button type="submit" disabled={isRunning}>
          Run
        </button>{" "}
        <button type="button" onClick={() => running.current?.abort()}>
          Stop
        </button>
      </form>
      <pre aria-live="polite">{output}</pre>
      <p>{status}</p>
      {approval && (
        <div role="dialog" aria-label="Approve change" className="approval">
          <p>
            Rename the family from "{approval.request.from}" to "
            {approval.request.to}"?
          </p>
          <button type="button" onClick={() => approval.answer(true)}>
            Approve
          </button>{" "}
          <button type="button" onClick={() => approval.answer(false)}>
            Decline
          </button>
        </div>
      )}
    </main>
  );
}
