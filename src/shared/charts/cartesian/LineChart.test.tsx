// @vitest-environment happy-dom

import { act, type ReactElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ChartThemeContextProvider, type ChartThemeValue } from "@/shared/charts/core/theme";
import type { XYPoint } from "@/shared/types/visualization/chartModel";
import { LineChart } from "./LineChart";

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

const points: XYPoint[] = [
  { x: 0, y: 2 },
  { x: 1, y: 5 },
  { x: 2, y: 3 },
];

let host: HTMLDivElement;
let root: Root;
let pendingFrames: Map<number, FrameRequestCallback>;
let nextFrameId: number;

function renderChart(chart: ReactElement): void {
  act(() => {
    root.render(<ChartThemeContextProvider value={chartTheme}>{chart}</ChartThemeContextProvider>);
  });

  const frames = [...pendingFrames.values()];
  pendingFrames.clear();
  act(() => frames.forEach((callback) => callback(0)));
}

beforeEach(() => {
  pendingFrames = new Map();
  nextFrameId = 0;
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

describe("LineChart", () => {
  it("keeps one line path and joins point circles from showPoints", () => {
    const chart = (showPoints: boolean) => (
      <LineChart
        data={points}
        xAxis={{ label: "Time", valueType: "number" }}
        yAxis={{ label: "Value", valueType: "number" }}
        height={280}
        showPoints={showPoints}
      />
    );

    renderChart(chart(false));

    expect(host.querySelectorAll('[data-chart-mark="line-path"]')).toHaveLength(1);
    expect(host.querySelectorAll('[data-chart-mark="line-point"]')).toHaveLength(0);

    act(() => {
      root.render(
        <ChartThemeContextProvider value={chartTheme}>{chart(true)}</ChartThemeContextProvider>,
      );
    });

    expect(host.querySelectorAll('[data-chart-mark="line-path"]')).toHaveLength(1);
    expect(host.querySelectorAll('[data-chart-mark="line-point"]')).toHaveLength(points.length);
  });

  it("formats date-axis ticks through the axis model", () => {
    renderChart(
      <LineChart
        data={points}
        xAxis={{ label: "Date", valueType: "date" }}
        yAxis={{ label: "Value", valueType: "number" }}
        height={280}
        showPoints={false}
      />,
    );

    const tickLabels = [...host.querySelectorAll('[data-chart-layer="x-axis"] .tick text')].map(
      (element) => element.textContent ?? "",
    );

    expect(tickLabels.some((label) => /^\d{4}-\d{2}-\d{2}$/.test(label))).toBe(true);
  });
});
