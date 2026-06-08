/**
 * Shared benchmark think budget — both engines stop on whichever limit hits first.
 */

export const BENCH_TIME_SEC = 10;
export const BENCH_MAX_SIMULATIONS = 2_000_000_000;
export const BENCH_TIME_MS = BENCH_TIME_SEC * 1000;

export function resolveThinkBudget(options = {}, playerConfig = {}) {
  return {
    timeSec: options.timeSec ?? playerConfig.timeSec ?? BENCH_TIME_SEC,
    timeMs:
      options.timeMs ??
      playerConfig.timeMs ??
      (options.timeSec ?? playerConfig.timeSec ?? BENCH_TIME_SEC) * 1000,
    maxSimulations:
      options.maxSimulations ??
      playerConfig.maxSimulations ??
      playerConfig.simulations ??
      BENCH_MAX_SIMULATIONS,
  };
}

export function formatThinkBudget(budget) {
  const sims =
    budget.maxSimulations >= 1_000_000_000
      ? `${(budget.maxSimulations / 1_000_000_000).toFixed(0)}B`
      : budget.maxSimulations.toLocaleString();
  return `${budget.timeSec}s / ${sims} sims cap`;
}
