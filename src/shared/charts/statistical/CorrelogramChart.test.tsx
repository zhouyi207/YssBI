// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ChartThemeContextProvider, type ChartThemeValue } from "@/shared/charts/core/theme";
import { CorrelogramChart } from "./CorrelogramChart";

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
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(480);
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(280);
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

describe("CorrelogramChart", () => {
  it("renders report bars without Ljung-Box fields", () => {
    act(() => {
      root.render(
        <ChartThemeContextProvider value={chartTheme}>
          <CorrelogramChart
            data={[
              { lag: 0, value: 1 },
              { lag: 1, value: 0.25 },
            ]}
            ciHalfWidth={0.2}
            valueLabel="ACF"
          />
        </ChartThemeContextProvider>,
      );
    });
    flushMeasurement();

    const chart = host.querySelector("svg");
    expect.soft(chart?.getAttribute("role")).toBe("group");
    expect.soft(chart?.getAttribute("aria-label")).toBe("ACF correlogram");

    const bars = [...host.querySelectorAll<SVGRectElement>('[data-chart-mark="correlogram-bar"]')];
    expect.soft(bars).toHaveLength(2);
    expect.soft(bars.map((bar) => bar.getAttribute("tabindex"))).toEqual(["0", "0"]);
    expect.soft(bars[0]?.getAttribute("aria-label")).toContain("Lag 0");
    expect.soft(bars[0]?.getAttribute("aria-label")).toContain("ACF 1.0000");
    expect.soft(bars[1]?.getAttribute("aria-label")).toContain("Lag 1");
    expect.soft(bars[1]?.getAttribute("aria-label")).toContain("ACF 0.2500");
    expect.soft(bars[1]?.getAttribute("aria-label")).not.toContain("Q(");
    expect.soft(bars[1]?.getAttribute("aria-label")).not.toContain("p-value");
  });

  it("exposes Plot statistics and updates supplied CI references with stable joins", () => {
    const data = [{ lag: 2, value: -0.375, qStat: 1.23456, pValue: 0.03 }];
    const render = (ciHalfWidth: number, nextData = data) => {
      act(() => {
        root.render(
          <ChartThemeContextProvider value={chartTheme}>
            <CorrelogramChart data={nextData} ciHalfWidth={ciHalfWidth} valueLabel="PACF" />
          </ChartThemeContextProvider>,
        );
      });
    };

    render(0.2);
    flushMeasurement();

    const bar = host.querySelector<SVGRectElement>('[data-chart-mark="correlogram-bar"]');
    expect.soft(bar?.getAttribute("aria-label")).toContain("Lag 2");
    expect.soft(bar?.getAttribute("aria-label")).toContain("PACF -0.3750");
    expect.soft(bar?.getAttribute("aria-label")).toContain("Q(2) 1.2346");
    expect.soft(bar?.getAttribute("aria-label")).toContain("p-value 0.0300");

    bar?.dispatchEvent(new FocusEvent("focus"));
    const tooltip = host.querySelector<HTMLDivElement>("svg + div");
    expect.soft(tooltip?.style.opacity).toBe("1");
    expect.soft(tooltip?.textContent).toContain("Q(2)");
    expect.soft(tooltip?.textContent).toContain("1.2346");
    expect.soft(tooltip?.textContent).toContain("p-value");
    expect.soft(tooltip?.textContent).toContain("0.0300");

    const upper = host.querySelector<SVGLineElement>(
      '[data-chart-reference="confidence"][data-ci-bound="upper"]',
    );
    const lower = host.querySelector<SVGLineElement>(
      '[data-chart-reference="confidence"][data-ci-bound="lower"]',
    );
    const region = host.querySelector<SVGRectElement>('[data-chart-region="confidence"]');
    const upperY = upper?.getAttribute("y1");
    const lowerY = lower?.getAttribute("y1");
    expect.soft(Number(upper?.getAttribute("data-chart-value"))).toBe(0.2);
    expect.soft(Number(lower?.getAttribute("data-chart-value"))).toBe(-0.2);

    render(0.4);

    const nextUpper = host.querySelector<SVGLineElement>(
      '[data-chart-reference="confidence"][data-ci-bound="upper"]',
    );
    const nextLower = host.querySelector<SVGLineElement>(
      '[data-chart-reference="confidence"][data-ci-bound="lower"]',
    );
    expect.soft(nextUpper).toBe(upper);
    expect.soft(nextLower).toBe(lower);
    expect.soft(host.querySelector('[data-chart-region="confidence"]')).toBe(region);
    expect.soft(host.querySelector('[data-chart-mark="correlogram-bar"]')).toBe(bar);
    expect.soft(Number(nextUpper?.getAttribute("data-chart-value"))).toBe(0.4);
    expect.soft(Number(nextLower?.getAttribute("data-chart-value"))).toBe(-0.4);
    expect.soft(nextUpper?.getAttribute("y1")).not.toBe(upperY);
    expect.soft(nextLower?.getAttribute("y1")).not.toBe(lowerY);
    expect.soft(tooltip?.style.opacity).toBe("0");

    render(0.4, []);
    expect.soft(host.querySelector('[data-chart-region="confidence"]')).toBeNull();
    expect.soft(host.querySelector('[data-chart-reference="confidence"]')).toBeNull();
    expect.soft(host.querySelector('[data-chart-reference="zero"]')).not.toBeNull();

    render(0.4);
    const grid = host.querySelector('[data-chart-layer="grid"]');
    const paintOrder = [...(grid?.children ?? [])];
    const recoveredRegion = host.querySelector('[data-chart-region="confidence"]');
    const recoveredConfidence = [...host.querySelectorAll('[data-chart-reference="confidence"]')];
    const recoveredZero = host.querySelector('[data-chart-reference="zero"]');
    expect
      .soft(paintOrder.indexOf(recoveredZero as Element))
      .toBeGreaterThan(paintOrder.indexOf(recoveredRegion as Element));
    for (const reference of recoveredConfidence) {
      expect
        .soft(paintOrder.indexOf(recoveredZero as Element))
        .toBeGreaterThan(paintOrder.indexOf(reference));
    }
  });
});
