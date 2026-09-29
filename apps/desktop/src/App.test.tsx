import { cleanup, render, screen } from "@testing-library/react";
import { commands } from "@typefaced/bindings";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "./App";

vi.mock("@typefaced/bindings", () => ({
  commands: { appInfo: vi.fn() },
}));

const appInfo = vi.mocked(commands.appInfo);

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

  it("shows the reason when Tauri rejects with a plain string", async () => {
    appInfo.mockRejectedValue("command app_info not found");

    render(<App />);

    expect((await screen.findByRole("alert")).textContent).toBe(
      "Could not read the app version: command app_info not found",
    );
  });
});
