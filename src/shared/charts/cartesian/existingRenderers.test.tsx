// @vitest-environment happy-dom

import { act, type ReactElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ChartThemeContextProvider, type ChartThemeValue } from "@/shared/charts/core/theme";
import { KdeChart } from "./KdeChart";
import { MultiLineChart } from "./MultiLineChart";
import { PredictiveIntervalChart } from "../statistical/PredictiveIntervalChart";

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

describe("existing shared chart renderers", () => {
  it("renders a non-empty KDE area and density line", () => {
    renderChart(
      <KdeChart
        data={[
          { x: 0, y: 0.001 },
          { x: 1, y: 0.002 },
          { x: 2, y: 0.0015 },
        ]}
      />,
    );

    const areas = host.querySelectorAll<SVGPathElement>('[data-chart-mark="kde-area"]');
    const densityLines = host.querySelectorAll<SVGPathElement>('[data-chart-mark="kde-line"]');

    expect(areas).toHaveLength(1);
    expect(densityLines).toHaveLength(1);
    expect(areas.item(0).getAttribute("d")).toBeTruthy();
    const densityPath = densityLines.item(0).getAttribute("d");
    expect(densityPath).toBeTruthy();

    const coordinates = densityPath?.match(/-?\d+(?:\.\d+)?/g)?.map(Number) ?? [];
    const yCoordinates = coordinates.filter((_, index) => index % 2 === 1);
    expect(Math.max(...yCoordinates) - Math.min(...yCoordinates)).toBeGreaterThan(100);
  });

  it("renders one line per non-empty series and keeps its legend labels", () => {
    renderChart(
      <MultiLineChart
        series={[
          {
            id: "chain-1",
            label: "Chain 1",
            points: [
              { x: 0, y: 1 },
              { x: 1, y: 2 },
            ],
          },
          {
            id: "chain-2",
            label: "Chain 2",
            points: [
              { x: 0, y: 2 },
              { x: 1, y: 3 },
            ],
          },
          { id: "empty", label: "Empty", points: [] },
        ]}
      />,
    );

    expect(host.querySelectorAll('[data-chart-mark="series"]')).toHaveLength(2);
    const legendText = host.querySelector('[aria-label="Chart legend"]')?.textContent;
    expect(legendText).toContain("Chain 1");
    expect(legendText).toContain("Chain 2");
    expect(legendText).not.toContain("Empty");
  });

  it("renders valid multi-line data after an initially empty render", () => {
    renderChart(<MultiLineChart series={[]} />);
    expect(host.querySelector('[data-chart-mark="series"]')).toBeNull();

    renderChart(
      <MultiLineChart
        series={[
          {
            id: "chain-1",
            label: "Chain 1",
            points: [
              { x: 0, y: 1 },
              { x: 1, y: 2 },
            ],
          },
        ]}
      />,
    );

    expect(host.querySelectorAll('[data-chart-mark="series"]')).toHaveLength(1);
    expect(host.querySelector("svg")?.getAttribute("width")).toBe("640");
  });

  it("renders interval, mean, and observed mark layers", () => {
    renderChart(
      <PredictiveIntervalChart
        data={[
          { observation: 1, observed: 1.2, mean: 1, lower: 0.7, upper: 1.3 },
          { observation: 2, observed: 1.8, mean: 2, lower: 1.5, upper: 2.4 },
        ]}
      />,
    );

    const interval = host.querySelector('[data-chart-mark-layer="interval"]');
    const mean = host.querySelector('[data-chart-mark-layer="mean"]');
    const observed = host.querySelector('[data-chart-mark-layer="observed"]');

    expect(interval?.querySelector("path")?.getAttribute("d")).toBeTruthy();
    expect(mean?.querySelector("path")?.getAttribute("d")).toBeTruthy();
    expect(observed?.querySelectorAll("circle")).toHaveLength(2);
  });
});
