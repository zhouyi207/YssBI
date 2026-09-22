import { describe, expect, it } from "vitest";
import { coerceFiniteNumber, formatNum, formatNullableNum, formatPercent } from "./formatStat";

describe("formatStat", () => {
  it("coerceFiniteNumber rejects nested objects", () => {
    expect(coerceFiniteNumber({ d: 1.85 })).toBeNull();
    expect(coerceFiniteNumber([1, 2])).toBeNull();
    expect(coerceFiniteNumber(1.25)).toBe(1.25);
  });

  it("formatNum never throws on invalid input", () => {
    expect(formatNum({ d: 1 })).toBe("—");
    expect(formatNum(undefined)).toBe("—");
    expect(formatNum(0.123456)).toBe("0.1235");
  });

  it("formatNum handles infinity", () => {
    expect(formatNum(Infinity)).toBe("Inf");
  });

  it("formatNullableNum preserves explicit fallback", () => {
    expect(formatNullableNum(null)).toBe("—");
    expect(formatNullableNum(undefined, 2, "N/A")).toBe("N/A");
  });

  it("formatPercent scales finite values", () => {
    expect(formatPercent(0.4567)).toBe("45.67%");
    expect(formatPercent({})).toBe("—");
  });
});
