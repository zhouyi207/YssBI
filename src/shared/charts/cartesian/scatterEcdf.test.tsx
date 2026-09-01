// @vitest-environment happy-dom

import { act, type ReactElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ChartThemeContextProvider, type ChartThemeValue } from "@/shared/charts/core/theme";
import type { XYPoint } from "@/shared/types/visualization/chartModel";
import { EcdfChart } from "./EcdfChart";
import { ScatterChart } from "./ScatterChart";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const chartTheme: ChartThemeValue = {
  colors: {
    canvas: "#ffffff",
    grid: "#e5e7eb",
    axis: "#9ca3af",
    tick: "#6b7280",
    label: "#374151",
    zeroLine: "#111827",
    tooltipBg: "#111827",
    tooltipFg: "#ffffff",
    tooltipMuted: "#d1d5db",
  },
  series: {
    primary: "#2563eb",
    negative: "#dc2626",
    secondary: "#d97706",
    highlight: "#dc2626",
    palette: ["#2563eb", "#dc2626", "#16a34a"],
  },
};

class TestResizeObserver implements ResizeObserver {
  readonly observe = vi.fn();
  readonly unobserve = vi.fn();
  readonly disconnect = vi.fn();

  constructor(_callback: ResizeObserverCallback) {}
}

let host: HTMLDivElement;
let root: Root;
let nextFrameId: number;
let pendingFrames: Map<number, FrameRequestCallback>;

function renderChart(chart: ReactElement): void {
  act(() => {
    root.render(<ChartThemeContextProvider value={chartTheme}>{chart}</ChartThemeContextProvider>);
  });

  const frames = [...pendingFrames.values()];
  pendingFrames.clear();
  act(() => frames.forEach((callback) => callback(0)));
}

beforeEach(() => {
  nextFrameId = 0;
  pendingFrames = new Map();
  vi.stubGlobal("ResizeObserver", TestResizeObserver);
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
    const frameId = ++nextFrameId;
    pendingFrames.set(frameId, callback);
    return frameId;
  });
  vi.stubGlobal("cancelAnimationFrame", (frameId: number) => {
    pendingFrames.delete(frameId);
  });
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(640);
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(320);
  host = document.createElement("div");
  document.body.appendChild(host);
  root = createRoot(host);
});

afterEach(() => {
  act(() => root.unmount());
  host.remove();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("scatter and ECDF cartesian renderers", () => {
  it("renders residual scatter semantics", () => {
    const data: XYPoint[] = [
      { x: 1, y: -2 },
      { x: 2, y: 4 },
      { x: 3, y: 1 },
    ];

    renderChart(
      <ScatterChart
        data={data}
        xAxis={{ label: "Fitted Values", valueType: "number" }}
        yAxis={{ label: "Residuals", valueType: "number" }}
        height={280}
        symmetricY
        zeroLine
        highlightIndices={new Set([1])}
      />,
    );

    const svg = host.querySelector("svg");
    const points = host.querySelectorAll<SVGCircleElement>('[data-chart-mark="scatter-point"]');
    const yDomain = JSON.parse(svg?.getAttribute("data-chart-y-domain") ?? "[]") as number[];

    expect(points).toHaveLength(data.length);
    expect(svg?.getAttribute("data-chart-x-domain")).toBeTruthy();
    expect(yDomain).toHaveLength(2);
    expect(yDomain[0]).toBeCloseTo(-yDomain[1]);
    expect(host.querySelectorAll('[data-chart-reference="zero"]')).toHaveLength(1);
    expect(points.item(0).getAttribute("data-highlighted")).toBe("false");
    expect(points.item(1).getAttribute("data-highlighted")).toBe("true");
    expect(points.item(1).getAttribute("fill")).toBe(chartTheme.series.highlight);
    expect(points.item(1).getAttribute("stroke")).toBe(chartTheme.series.highlight);
  });

  it("renders ECDF as one step-after path without re-sorting canonical data", () => {
    const data: XYPoint[] = [
      { x: 1, y: 0.25 },
      { x: 2, y: 0.5 },
      { x: 3, y: 0.75 },
      { x: 4, y: 1 },
    ];
    const originalData = data.map((point) => ({ ...point }));
    Object.freeze(data);

    renderChart(
      <EcdfChart
        data={data}
        xAxis={{ label: "Value", valueType: "number" }}
        yAxis={{ label: "Cumulative Proportion", valueType: "number" }}
        height={280}
      />,
    );

    const paths = host.querySelectorAll<SVGPathElement>('[data-chart-mark="ecdf-path"]');

    expect(paths).toHaveLength(1);
    expect(paths.item(0).getAttribute("data-chart-curve")).toBe("step-after");
    expect(paths.item(0).getAttribute("d")).toBeTruthy();
    expect(data).toEqual(originalData);
  });
});
