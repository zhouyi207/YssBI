// @vitest-environment happy-dom

import { act, type ReactElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ChartThemeContextProvider, type ChartThemeValue } from "@/shared/charts/core/theme";
import { KdeChart } from "./KdeChart";

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
});
