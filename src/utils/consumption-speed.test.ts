import { describe, expect, it } from "vitest";
import { evaluateConsumptionSpeed } from "./consumption-speed";

const minute = 60_000;

describe("evaluateConsumptionSpeed", () => {
  it("returns unknown without a long enough sample interval", () => {
    expect(evaluateConsumptionSpeed([
      { t: 0, fiveHour: 90 },
      { t: 10 * minute, fiveHour: 88 },
    ], "fiveHour").state).toBe("unknown");
  });

  it("classifies a sustainable five-hour pace as steady", () => {
    const result = evaluateConsumptionSpeed([
      { t: 0, fiveHour: 90 },
      { t: 60 * minute, fiveHour: 70 },
    ], "fiveHour");
    expect(result.state).toBe("steady");
    expect(result.percentPerHour).toBe(20);
  });

  it("classifies rapid consumption as critical", () => {
    expect(evaluateConsumptionSpeed([
      { t: 0, fiveHour: 90 },
      { t: 30 * minute, fiveHour: 60 },
    ], "fiveHour").state).toBe("critical");
  });

  it("does not calculate across a quota reset", () => {
    expect(evaluateConsumptionSpeed([
      { t: 0, fiveHour: 20 },
      { t: 30 * minute, fiveHour: 95 },
    ], "fiveHour").state).toBe("unknown");
  });
});
