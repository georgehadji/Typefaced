/** Distribution of a set of timings, in milliseconds. */
export interface Summary {
  count: number;
  mean: number;
  min: number;
  p50: number;
  p95: number;
  p99: number;
  max: number;
}

/** Nearest-rank percentile of an ascending array (`p` in 0..100). */
export function percentile(sorted: readonly number[], p: number): number {
  if (sorted.length === 0) {
    throw new RangeError("percentile of an empty sample");
  }
  const rank = Math.ceil((p / 100) * sorted.length);
  return sorted[Math.min(Math.max(rank, 1), sorted.length) - 1];
}

/** Summarises timings; throws on an empty sample, which means a broken run. */
export function summarize(samples: readonly number[]): Summary {
  const sorted = [...samples].sort((a, b) => a - b);
  const sum = sorted.reduce((total, x) => total + x, 0);
  return {
    count: sorted.length,
    mean: sum / Math.max(sorted.length, 1),
    min: percentile(sorted, 0),
    p50: percentile(sorted, 50),
    p95: percentile(sorted, 95),
    p99: percentile(sorted, 99),
    max: percentile(sorted, 100),
  };
}
