// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ChartModel } from "@/shared/types/visualization/chartModel";
import { ChartRenderer } from "./ChartRenderer";

const leafCalls = vi.hoisted(() => ({
  scatter: vi.fn(),
  line: vi.fn(),
  histogram: vi.fn(),
  ecdf: vi.fn(),
  kde: vi.fn(),
  correlation: vi.fn(),
  correlogram: vi.fn(),
}));

vi.mock("./cartesian/ScatterChart", () => ({
  ScatterChart: (props: unknown) => {
    leafCalls.scatter(props);
    return <div data-chart-leaf="scatter" />;
  },
}));
vi.mock("./cartesian/LineChart", () => ({
  LineChart: (props: unknown) => {
    leafCalls.line(props);
    return <div data-chart-leaf="line" />;
  },
}));
vi.mock("./cartesian/HistogramChart", () => ({
  HistogramChart: (props: unknown) => {
    leafCalls.histogram(props);
    return <div data-chart-leaf="histogram" />;
  },
}));
vi.mock("./cartesian/EcdfChart", () => ({
  EcdfChart: (props: unknown) => {
    leafCalls.ecdf(props);
    return <div data-chart-leaf="ecdf" />;
  },
}));
vi.mock("./cartesian/KdeChart", () => ({
  KdeChart: (props: unknown) => {
    leafCalls.kde(props);
    return <div data-chart-leaf="kde" />;
  },
}));
vi.mock("./statistical/CorrelationMatrixChart", () => ({
  CorrelationMatrixChart: (props: unknown) => {
    leafCalls.correlation(props);
    return <div data-chart-leaf="correlation" />;
  },
}));
vi.mock("./statistical/CorrelogramChart", () => {
  const CorrelogramChart = (props: unknown) => {
    leafCalls.correlogram(props);
    return <div data-chart-leaf="correlogram" />;
  };
  return { CorrelogramChart, default: CorrelogramChart };
});
vi.mock("./core/theme", () => ({
  useChartTheme: () => ({ series: { secondary: "secondary-series" } }),
}));

type ChartModelKind = ChartModel["kind"];
type ChartModelFixtures = {
  [K in ChartModelKind]: Extract<ChartModel, { kind: K }>;
};
type LeafName = keyof typeof leafCalls;

const models = {
  scatter: {
    kind: "scatter",
    points: [{ x: 1, y: 2 }],
    xAxis: { label: "Scatter X", valueType: "number" },
    yAxis: { label: "Scatter Y", valueType: "number" },
  },
  line: {
    kind: "line",
    points: [{ x: 3, y: 4 }],
    xAxis: { label: "Line X", valueType: "date" },
    yAxis: { label: "Line Y", valueType: "datetime" },
    showPoints: false,
  },
  histogram: {
    kind: "histogram",
    bins: [{ label: "0–1", count: 2 }],
    xLabel: "Bins",
    yLabel: "Count",
  },
  ecdf: {
    kind: "ecdf",
    points: [{ x: 5, y: 0.5 }],
    xAxis: { label: "ECDF X", valueType: "number" },
    yAxis: { label: "ECDF Y", valueType: "number" },
  },
  kde: {
    kind: "kde",
    points: [{ x: 6, y: 0.25 }],
    xAxis: { label: "KDE X", valueType: "number" },
    yAxis: { label: "KDE Y", valueType: "number" },
    xMin: 0,
  },
  correlation: {
    kind: "correlation",
    labels: ["a", "b"],
    matrix: [
      [1, 0.5],
      [0.5, 1],
    ],
    pMatrix: [
      [0, 0.1],
      [0.1, 0],
    ],
  },
  correlogram: {
    kind: "correlogram",
    acf: [{ lag: 0, value: 1, qStat: 0, pValue: 1 }],
    pacf: [{ lag: 1, value: 0.4, qStat: 1.2, pValue: 0.3 }],
    ciHalfWidth: 0.2,
  },
} satisfies ChartModelFixtures;

const expectedLeaves = {
  scatter: ["scatter"],
  line: ["line"],
  histogram: ["histogram"],
  ecdf: ["ecdf"],
  kde: ["kde"],
  correlation: ["correlation"],
  correlogram: ["correlogram", "correlogram"],
} satisfies { [K in ChartModelKind]: readonly LeafName[] };

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

let host: HTMLDivElement;
let root: Root;

beforeEach(() => {
  vi.clearAllMocks();
  host = document.createElement("div");
  document.body.appendChild(host);
  root = createRoot(host);
});

afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

describe("ChartRenderer", () => {
  it("routes every chart model kind to exactly its registered leaf renderer", () => {
    for (const kind of Object.keys(models) as ChartModelKind[]) {
      vi.clearAllMocks();
      act(() => root.render(<ChartRenderer model={models[kind]} surface="plain" />));

      const renderedLeaves = [...host.querySelectorAll<HTMLElement>("[data-chart-leaf]")].map(
        (element) => element.dataset.chartLeaf,
      );
      expect(renderedLeaves).toEqual(expectedLeaves[kind]);

      for (const [leaf, calls] of Object.entries(leafCalls) as Array<
        [LeafName, (typeof leafCalls)[LeafName]]
      >) {
        expect(calls).toHaveBeenCalledTimes(
          expectedLeaves[kind].filter((expected) => expected === leaf).length,
        );
      }
    }
  });

  it("forwards line point visibility and axis formats", () => {
    act(() => root.render(<ChartRenderer model={models.line} />));

    expect(leafCalls.line).toHaveBeenCalledWith({
      data: models.line.points,
      xAxis: { label: "Line X", valueType: "date" },
      yAxis: { label: "Line Y", valueType: "datetime" },
      showPoints: false,
    });
  });
});
