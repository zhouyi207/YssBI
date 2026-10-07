// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ChartThemeContextProvider, type ChartThemeValue } from "@/shared/charts/core/theme";
import { HistogramChart } from "./HistogramChart";

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
let pendingFrames: Map<number, FrameRequestCallback>;
let nextFrameId: number;

function flushMeasurement(): void {
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
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(320);
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(200);
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

describe("HistogramChart", () => {
  it("keeps all-zero bins at zero height", () => {
    act(() => {
      root.render(
        <ChartThemeContextProvider value={chartTheme}>
          <HistogramChart
            data={[
              { label: "A", count: 0 },
              { label: "B", count: 0 },
            ]}
            height={200}
          />
        </ChartThemeContextProvider>,
      );
    });
    flushMeasurement();

    const heights = [...host.querySelectorAll<SVGRectElement>("rect.bar")].map((bar) =>
      Number(bar.getAttribute("height")),
    );

    expect(heights).toEqual([0, 0]);
  });

  it("uses group semantics only for keyboard-interactive compact bars", () => {
    const data = [
      { label: "A", count: 2 },
      { label: "B", count: 3 },
    ];
    const render = (compact: boolean) => {
      act(() => {
        root.render(
          <ChartThemeContextProvider value={chartTheme}>
            <HistogramChart
              data={data}
              xLabel="Value"
              yLabel="Count"
              height={200}
              compact={compact}
            />
          </ChartThemeContextProvider>,
        );
      });
    };

    render(false);
    flushMeasurement();

    let svg = host.querySelector("svg");
    let bars = [...host.querySelectorAll<SVGRectElement>("rect.bar")];
    expect.soft(svg?.getAttribute("role")).toBe("img");
    expect.soft(svg?.getAttribute("aria-label")).toBe("Count histogram by Value");
    expect.soft(bars.map((bar) => bar.getAttribute("tabindex"))).toEqual([null, null]);
    expect.soft(bars.map((bar) => bar.getAttribute("aria-label"))).toEqual([null, null]);

    render(true);

    svg = host.querySelector("svg");
    bars = [...host.querySelectorAll<SVGRectElement>("rect.bar")];
    expect.soft(svg?.getAttribute("role")).toBe("group");
    expect.soft(svg?.getAttribute("aria-label")).toBe("Count histogram by Value");
    expect.soft(bars.map((bar) => bar.getAttribute("tabindex"))).toEqual(["0", "0"]);
    expect
      .soft(bars.map((bar) => bar.getAttribute("aria-label")))
      .toEqual(["Histogram bin A, Count 2", "Histogram bin B, Count 3"]);
  });
});
