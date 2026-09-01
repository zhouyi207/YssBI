import { describe, expect, it } from "vitest";
import fixture from "@/tests/fixtures/node-system-contracts/plot-payloads.json";
import type { ResultPlotKind } from "./result";
import {
  parseCorrelationPlot,
  parseCorrelogramPlot,
  parseHistogramPlot,
  parsePlotPayload,
  parseXySeriesPlot,
} from "./plotPayload";

const fixtureByKind = Object.fromEntries(
  fixture.payloads.map((record) => [record.chart, record.data]),
);

describe("Rust plot payload contract", () => {
  it("parses every current production plot kind", () => {
    expect(fixture.payloads.map((record) => record.chart).sort()).toEqual([
      "correlation",
      "correlogram",
      "ecdf",
      "histogram",
      "kde",
      "line",
      "scatter",
    ]);
    for (const record of fixture.payloads) {
      const parsed = parsePlotPayload(record.chart as ResultPlotKind, record.data);
      expect(parsed?.kind).toBe(record.chart);
    }
  });

  it("preserves correlation and correlogram statistics", () => {
    const correlation = parsePlotPayload("correlation", fixtureByKind.correlation);
    expect(correlation?.kind === "correlation" && correlation.data.pMatrix?.[0]?.[1]).toBe(0.25);

    const correlogram = parsePlotPayload("correlogram", fixtureByKind.correlogram);
    expect(correlogram?.kind === "correlogram" && correlogram.data.ciHalfWidth).toBeGreaterThan(0);
    expect(correlogram?.kind === "correlogram" && correlogram.data.acf[0]?.qStat).toBe(1.5);
    expect(correlogram?.kind === "correlogram" && correlogram.data.acf[0]?.pValue).toBe(0.2);
  });
});

describe("parseXySeriesPlot", () => {
  it("accepts camelCase scatter payload from Rust", () => {
    const result = parseXySeriesPlot({
      data: [{ x: 1, y: 2 }],
      xLabel: "X",
      yLabel: "Y",
      xFormat: "date",
      yFormat: "number",
    });
    expect(result).toEqual({
      data: [{ x: 1, y: 2 }],
      xLabel: "X",
      yLabel: "Y",
      xFormat: "date",
      yFormat: "number",
    });
  });

  it("does not read snake_case axis metadata", () => {
    const result = parseXySeriesPlot({
      data: [{ x: 0.5, y: 0.25 }],
      x_label: "Value",
      y_label: "ECDF",
    });
    expect(result?.xLabel).toBeUndefined();
    expect(result?.yLabel).toBeUndefined();
  });

  it("rejects empty or invalid points", () => {
    expect(parseXySeriesPlot({ data: [] })).toBeNull();
    expect(parseXySeriesPlot({ data: [{ x: "a", y: 1 }] })).toBeNull();
    expect(parseXySeriesPlot(null)).toBeNull();
  });
});

describe("parseHistogramPlot", () => {
  it("parses histogram bins", () => {
    const result = parseHistogramPlot({
      data: [{ label: "[0, 1)", count: 3 }],
      xLabel: "x",
      yLabel: "Frequency",
    });
    expect(result).toEqual({
      data: [{ label: "[0, 1)", count: 3 }],
      xLabel: "x",
      yLabel: "Frequency",
    });
  });

  it("rejects non-integer counts", () => {
    expect(
      parseHistogramPlot({
        data: [{ label: "a", count: 1.5 }],
      }),
    ).toBeNull();
  });
});

describe("parseCorrelogramPlot", () => {
  it("parses acf/pacf with ci and n", () => {
    const result = parseCorrelogramPlot({
      acf: [{ lag: 1, value: 0.5, qStat: 1.2, pValue: 0.3 }],
      pacf: [{ lag: 1, value: 0.4, qStat: 1.1, pValue: 0.25 }],
      ciHalfWidth: 0.2,
      n: 100,
    });
    expect(result).toEqual({
      acf: [{ lag: 1, value: 0.5, qStat: 1.2, pValue: 0.3 }],
      pacf: [{ lag: 1, value: 0.4, qStat: 1.1, pValue: 0.25 }],
      ciHalfWidth: 0.2,
      n: 100,
    });
  });

  it("rejects plot bar missing ljung-box stats", () => {
    expect(
      parseCorrelogramPlot({
        acf: [{ lag: 1, value: 0.5 }],
        pacf: [{ lag: 1, value: 0.4, qStat: 1, pValue: 0.1 }],
        ciHalfWidth: 0.2,
        n: 100,
      }),
    ).toBeNull();
  });
});

describe("parseCorrelationPlot", () => {
  it("parses nullable square matrices with optional pMatrix", () => {
    const result = parseCorrelationPlot({
      labels: ["A", "B"],
      matrix: [
        [1, 0.5],
        [null, 1],
      ],
      pMatrix: [
        [0, 0.1],
        [null, 0],
      ],
    });
    expect(result?.labels).toEqual(["A", "B"]);
    expect(result?.matrix[1]?.[0]).toBeNull();
    expect(result?.pMatrix?.[0][1]).toBe(0.1);
  });

  it("rejects non-square matrix", () => {
    expect(
      parseCorrelationPlot({
        labels: ["A", "B"],
        matrix: [[1]],
      }),
    ).toBeNull();
  });
});

describe("parsePlotPayload", () => {
  it("dispatches by chart kind", () => {
    expect(
      parsePlotPayload("scatter", {
        data: [{ x: 1, y: 2 }],
      })?.kind,
    ).toBe("scatter");
    expect(
      parsePlotPayload("histogram", {
        data: [{ label: "a", count: 1 }],
      })?.kind,
    ).toBe("histogram");
    expect(parsePlotPayload("scatter", { data: [] })).toBeNull();
  });
});
