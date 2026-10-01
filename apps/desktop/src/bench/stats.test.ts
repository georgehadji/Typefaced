import { describe, expect, it } from "vitest";
import { percentile, summarize } from "./stats";

describe("percentile", () => {
  const sorted = Array.from({ length: 100 }, (_, i) => i + 1);

  it("uses the nearest rank", () => {
    expect(percentile(sorted, 50)).toBe(50);
    expect(percentile(sorted, 95)).toBe(95);
    expect(percentile([1, 2, 3], 50)).toBe(2);
  });

  it("clamps 0 and 100 to the first and last sample", () => {
    expect(percentile(sorted, 0)).toBe(1);
    expect(percentile(sorted, 100)).toBe(100);
  });

  it("rejects an empty sample", () => {
    expect(() => percentile([], 50)).toThrow(RangeError);
  });
});

describe("summarize", () => {
  it("sorts a copy and reports the distribution", () => {
    const samples = [4, 1, 3, 2];
    expect(summarize(samples)).toEqual({
      count: 4,
      mean: 2.5,
      min: 1,
      p50: 2,
      p95: 4,
      p99: 4,
      max: 4,
    });
    expect(samples).toEqual([4, 1, 3, 2]);
  });

  it("throws on an empty sample", () => {
    expect(() => summarize([])).toThrow(RangeError);
  });
});
