import { describe, expect, it } from "vitest";
import { compactNumber } from "./formatters";

describe("compactNumber", () => {
  it("formats empty and small values", () => {
    expect(compactNumber(null)).toBe("—");
    expect(compactNumber(999)).toBe("999");
  });

  it("formats large values with compact units", () => {
    expect(compactNumber(1_500)).toBe("1.5k");
    expect(compactNumber(2_000_000)).toBe("2M");
  });
});
