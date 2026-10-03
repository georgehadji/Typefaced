import { hitTest } from "@typefaced/geometry-wasm";
import { afterEach, describe, expect, it, vi } from "vitest";
import { dataUrlBytes, loadKernel } from "./kernel";

describe("dataUrlBytes", () => {
  it("decodes a base64 data URL to its bytes", () => {
    expect([...dataUrlBytes("data:application/wasm;base64,AGFzbQE=")]).toEqual([
      0, 97, 115, 109, 1,
    ]);
  });

  it("rejects a string that is not a base64 data URL", () => {
    expect(() => dataUrlBytes("/assets/tf_wasm_bg.wasm")).toThrow(TypeError);
  });
});

describe("loadKernel", () => {
  afterEach(() => vi.restoreAllMocks());

  // The shipped CSP has no `'self'` in connect-src, so fetch() of the module would be blocked.
  it("loads the module from inlined bytes, without fetch, once", async () => {
    const fetchSpy = vi.spyOn(globalThis, "fetch");
    await loadKernel();
    await loadKernel();
    expect(fetchSpy).not.toHaveBeenCalled();
    const square = new Float64Array([0, 0, 10, 0, 10, 10, 0, 10]);
    const hit = hitTest(
      square,
      new Uint8Array([1, 1, 1, 1]),
      Uint32Array.of(3),
      9,
      9,
      3,
    );
    expect([...hit]).toEqual([0, 2, Math.SQRT2]);
  });
});
