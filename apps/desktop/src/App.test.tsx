import { cleanup, render, screen } from "@testing-library/react";
import { type AppInfo, commands } from "@typefaced/bindings";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "./App";

vi.mock("@typefaced/bindings", () => ({
  commands: { appInfo: vi.fn() },
}));

const appInfo = vi.mocked(commands.appInfo);

// A pending appInfo() call that the test settles by hand.
function deferred() {
  let resolve: (info: AppInfo) => void = () => {};
  let reject: (error: unknown) => void = () => {};
  const promise = new Promise<AppInfo>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe("App", () => {
  afterEach(() => {
    cleanup();
    appInfo.mockReset();
  });

  it("shows the name and version returned by the typed binding", async () => {
    appInfo.mockResolvedValue({ name: "Typefaced", version: "1.2.3" });

    render(<App />);

    expect(
      await screen.findByRole("heading", { name: "Typefaced v1.2.3" }),
    ).toBeTruthy();
  });

  it("shows an alert with the reason when the command fails", async () => {
    appInfo.mockRejectedValue(new Error("IPC unavailable"));

    render(<App />);

    expect((await screen.findByRole("alert")).textContent).toBe(
      "Could not read the app version: IPC unavailable",
    );
  });

  it("ignores a result that arrives after unmount", async () => {
    const { promise, resolve } = deferred();
    appInfo.mockReturnValue(promise);

    const { unmount } = render(<App />);
    unmount();
    resolve({ name: "Typefaced", version: "1.2.3" });
    await promise;

    expect(screen.queryByRole("heading")).toBeNull();
  });

  it("ignores a failure that arrives after unmount", async () => {
    const { promise, reject } = deferred();
    appInfo.mockReturnValue(promise);

    const { unmount } = render(<App />);
    unmount();
    reject(new Error("IPC unavailable"));
    await promise.catch(() => undefined);

    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("shows the reason when Tauri rejects with a plain string", async () => {
    appInfo.mockRejectedValue("command app_info not found");

    render(<App />);

    expect((await screen.findByRole("alert")).textContent).toBe(
      "Could not read the app version: command app_info not found",
    );
  });
});
