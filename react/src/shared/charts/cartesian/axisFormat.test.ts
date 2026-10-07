import { describe, expect, it } from "vitest";
import { numToPlotDate, plotAxisTickFormatter } from "./axisFormat";

describe("axisFormat", () => {
  it("numToPlotDate converts day and microsecond encodings", () => {
    expect(numToPlotDate(1, "date").getTime()).toBe(86400000);
    expect(numToPlotDate(1_000_000, "datetime").getTime()).toBe(1000);
  });

  it("plotAxisTickFormatter returns undefined for numeric axes", () => {
    expect(plotAxisTickFormatter("number")).toBeUndefined();
  });

  it("plotAxisTickFormatter formats date ticks", () => {
    const format = plotAxisTickFormatter("date");
    expect(format?.({ valueOf: () => 0 })).toBe("1970-01-01");
    expect(format?.({ valueOf: () => -1 })).toBe("1969-12-31");
    const datetime = plotAxisTickFormatter("datetime");
    expect(datetime?.({ valueOf: () => 0 })).toBe("1970-01-01 00:00");
    expect(datetime?.({ valueOf: () => -1_000_000 })).toBe("1969-12-31 23:59");
    expect(datetime?.({ valueOf: () => 36_000_000_000 })).toBe("1970-01-01 10:00");
  });
});
