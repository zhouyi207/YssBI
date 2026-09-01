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
    expect(format?.({ valueOf: () => 0 })).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });
});
