import type { HistoryPoint } from "../types/usage";

export type ConsumptionSpeedState = "unknown" | "idle" | "steady" | "fast" | "critical";
export type UsageWindowKey = "fiveHour" | "weekly";

export interface ConsumptionSpeed {
  state: ConsumptionSpeedState;
  percentPerHour: number | null;
  paceRatio: number | null;
}

const WINDOW_CONFIG: Record<UsageWindowKey, { hours: number; lookbackMs: number; minSampleMs: number }> = {
  fiveHour: { hours: 5, lookbackMs: 90 * 60_000, minSampleMs: 15 * 60_000 },
  weekly: { hours: 7 * 24, lookbackMs: 24 * 60 * 60_000, minSampleMs: 60 * 60_000 },
};

function pointValue(point: HistoryPoint, key: UsageWindowKey): number | null {
  const value = key === "fiveHour" ? (point.fiveHour ?? point.p) : point.weekly;
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

export function evaluateConsumptionSpeed(
  points: HistoryPoint[],
  key: UsageWindowKey,
): ConsumptionSpeed {
  const config = WINDOW_CONFIG[key];
  const samples = points
    .map((point) => ({ t: point.t, value: pointValue(point, key) }))
    .filter((point): point is { t: number; value: number } => point.value !== null)
    .sort((a, b) => a.t - b.t);

  const latest = samples[samples.length - 1];
  if (!latest) return { state: "unknown", percentPerHour: null, paceRatio: null };

  let baseline: { t: number; value: number } | null = null;
  for (let index = samples.length - 2; index >= 0; index -= 1) {
    const sample = samples[index];
    const elapsed = latest.t - sample.t;
    if (elapsed > config.lookbackMs) break;
    // 额度明显回升表示进入了新的重置周期，不跨周期计算速度。
    if (sample.value + 2 < latest.value) break;
    if (elapsed >= config.minSampleMs) baseline = sample;
  }

  if (!baseline) return { state: "unknown", percentPerHour: null, paceRatio: null };

  const elapsedHours = (latest.t - baseline.t) / 3_600_000;
  const consumed = Math.max(0, baseline.value - latest.value);
  const percentPerHour = consumed / elapsedHours;
  const sustainableRate = 100 / config.hours;
  const paceRatio = percentPerHour / sustainableRate;
  const state: ConsumptionSpeedState = paceRatio < 0.1
    ? "idle"
    : paceRatio <= 1
      ? "steady"
      : paceRatio <= 1.5
        ? "fast"
        : "critical";

  return { state, percentPerHour, paceRatio };
}
