export function compactNumber(value: number | null | undefined): string {
  if (value == null) return "—";
  for (const unit of [
    { threshold: 1_000_000_000, suffix: "B" },
    { threshold: 1_000_000, suffix: "M" },
    { threshold: 1_000, suffix: "k" },
  ]) {
    if (value >= unit.threshold) {
      const scaled = value / unit.threshold;
      return `${scaled.toFixed(scaled < 100 && !Number.isInteger(scaled) ? 1 : 0)}${unit.suffix}`;
    }
  }
  return String(Math.round(value));
}
