import { ToolError } from "@anthropic-ai/sdk/lib/tools/ToolError";
import { describe, expect, it, vi } from "vitest";
import { createFontStore, FIXTURE_FONT, fontTools } from "./fontTools";

function setup(approve = vi.fn().mockResolvedValue(true)) {
  const store = createFontStore(FIXTURE_FONT);
  const [summary, rename] = fontTools(store, approve);
  return { store, approve, summary, rename };
}

describe("fontTools", () => {
  it("defines two client tools with closed schemas", () => {
    const { summary, rename } = setup();
    expect(summary.name).toBe("get_font_summary");
    expect(rename.name).toBe("set_family_name");
    for (const tool of [summary, rename]) {
      expect(tool).toMatchObject({
        type: "custom",
        input_schema: { type: "object", additionalProperties: false },
      });
      expect(tool).not.toHaveProperty("eager_input_streaming");
      // The egress policy refuses any key named `source` (tf-ai-host body check).
      expect(JSON.stringify(tool)).not.toContain('"source"');
    }
  });

  it("get_font_summary returns the current font as JSON", async () => {
    const { summary } = setup();
    expect(JSON.parse(String(await summary.run({})))).toEqual(FIXTURE_FONT);
  });

  it("refuses input that does not match the schema, without running", async () => {
    const { store, approve, summary, rename } = setup();
    for (const input of [
      {},
      { family_name: 5 },
      { family_name: "" },
      { family_name: "x".repeat(64) },
      { family_name: "Ok", extra: true },
      // Control and format characters (a newline, a right-to-left override).
      { family_name: `Test${String.fromCharCode(0x0a)}Sans` },
      { family_name: `Test${String.fromCharCode(0x202e)}Sans` },
      "not an object",
    ]) {
      const result = rename.run(input as never);
      await expect(result).rejects.toBeInstanceOf(ToolError);
      await expect(result).rejects.toMatchObject({
        content: expect.stringContaining("INVALID_JSON"),
      });
    }
    await expect(summary.run({ extra: 1 } as never)).rejects.toBeInstanceOf(
      ToolError,
    );
    expect(approve).not.toHaveBeenCalled();
    expect(store.get()).toEqual(FIXTURE_FONT);
  });

  it("set_family_name changes nothing when the user declines", async () => {
    const { store, rename } = setup(vi.fn().mockResolvedValue(false));
    const result = await rename.run({ family_name: "Test Sans" });
    expect(result).toMatch(/declined/);
    expect(store.get()).toEqual(FIXTURE_FONT);
  });

  it("set_family_name asks for approval, then replaces the font", async () => {
    const { store, approve, rename } = setup();
    const before = store.get();
    const result = await rename.run({ family_name: "Test Sans" });
    expect(approve).toHaveBeenCalledWith({
      tool: "set_family_name",
      from: "Typefaced Test",
      to: "Test Sans",
    });
    expect(result).toMatch(/Test Sans/);
    expect(store.get()).toEqual({ ...FIXTURE_FONT, familyName: "Test Sans" });
    expect(before).toEqual(FIXTURE_FONT);
  });

  it("the store notifies subscribers of changes", () => {
    const store = createFontStore(FIXTURE_FONT);
    const listener = vi.fn();
    const unsubscribe = store.subscribe(listener);
    store.set({ ...FIXTURE_FONT, familyName: "A" });
    unsubscribe();
    store.set({ ...FIXTURE_FONT, familyName: "B" });
    expect(listener).toHaveBeenCalledTimes(1);
    expect(listener).toHaveBeenCalledWith({ ...FIXTURE_FONT, familyName: "A" });
  });
});
