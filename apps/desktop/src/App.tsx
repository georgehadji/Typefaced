import { commands } from "@typefaced/bindings";
import { useEffect, useState } from "react";
import "./App.css";

type Status =
  | { kind: "loading" }
  | { kind: "ready"; label: string }
  | { kind: "failed"; reason: string };

// Tauri rejects IPC calls with plain strings; other failures arrive as Errors.
function describeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export default function App() {
  const [status, setStatus] = useState<Status>({ kind: "loading" });

  useEffect(() => {
    let isMounted = true;
    commands
      .appInfo()
      .then((info) => {
        if (isMounted) {
          setStatus({ kind: "ready", label: `${info.name} v${info.version}` });
        }
      })
      .catch((error: unknown) => {
        if (isMounted) {
          setStatus({ kind: "failed", reason: describeError(error) });
        }
      });
    return () => {
      isMounted = false;
    };
  }, []);

  return (
    <main className="app">
      <h1>{status.kind === "ready" ? status.label : "Typefaced"}</h1>
      {status.kind === "failed" && (
        <p role="alert">Could not read the app version: {status.reason}</p>
      )}
    </main>
  );
}
