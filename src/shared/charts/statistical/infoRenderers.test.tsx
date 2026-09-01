// @vitest-environment happy-dom

import { act, type ReactNode } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ChartThemeContextProvider, type ChartThemeValue } from "@/shared/charts/core/theme";
import { DidEventStudyChart } from "./DidEventStudyChart";
import { VarStabilityChart, type VarStabilityPoint } from "./VarStabilityChart";

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

function renderChart(chart: ReactNode): void {
  act(() => {
    root.render(<ChartThemeContextProvider value={chartTheme}>{chart}</ChartThemeContextProvider>);
  });
  flushMeasurement();
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

describe("shared statistical info renderers", () => {
  it("renders computed DID coefficients, intervals, and reference lines", () => {
    renderChart(
      <DidEventStudyChart
        points={[
          {
            rel_time: -1,
            coef: -0.2,
            std_err: 0.1,
            ci_low: -0.4,
            ci_high: 0,
          },
          {
            rel_time: 0,
            coef: 0,
            std_err: 0,
            ci_low: 0,
            ci_high: 0,
            is_reference: true,
          },
          {
            rel_time: 1,
            coef: 0.3,
            std_err: 0.1,
            ci_low: 0.1,
            ci_high: 0.5,
          },
        ]}
        xLabel="相对政策时点"
        yLabel="处理效应系数"
        ariaLabel="事件研究系数图"
      />,
    );

    const svg = host.querySelector("svg");
    expect.soft(svg?.getAttribute("role")).toBe("img");
    expect.soft(svg?.getAttribute("aria-label")).toBe("事件研究系数图");
    expect.soft(host.textContent).toContain("相对政策时点");
    expect.soft(host.textContent).toContain("处理效应系数");

    const points = [
      ...host.querySelectorAll<SVGCircleElement>('[data-chart-mark="did-coefficient"]'),
    ];
    expect.soft(points).toHaveLength(3);
    expect
      .soft(points.map((point) => Number(point.getAttribute("data-rel-time"))))
      .toEqual([-1, 0, 1]);
    expect
      .soft(points.map((point) => Number(point.getAttribute("data-coefficient"))))
      .toEqual([-0.2, 0, 0.3]);

    const intervals = [
      ...host.querySelectorAll<SVGLineElement>('[data-chart-mark="did-confidence-interval"]'),
    ];
    expect.soft(intervals).toHaveLength(2);
    expect
      .soft(intervals.map((interval) => Number(interval.getAttribute("data-ci-low"))))
      .toEqual([-0.4, 0.1]);
    expect
      .soft(intervals.map((interval) => Number(interval.getAttribute("data-ci-high"))))
      .toEqual([0, 0.5]);

    expect.soft(host.querySelectorAll('[data-chart-reference="zero"]')).toHaveLength(1);
    expect.soft(host.querySelectorAll('[data-chart-reference="policy-time"]')).toHaveLength(1);
    expect.soft(host.querySelectorAll('[data-chart-mark="did-coefficient-trend"]')).toHaveLength(1);
  });

  it("renders DID data after an initially empty render", () => {
    renderChart(
      <DidEventStudyChart
        points={[]}
        xLabel="Relative time"
        yLabel="Coefficient"
        ariaLabel="Event study"
      />,
    );
    expect(host.querySelector('[data-chart-mark="did-coefficient"]')).toBeNull();

    renderChart(
      <DidEventStudyChart
        points={[
          {
            rel_time: 1,
            coef: 0.3,
            std_err: 0.1,
            ci_low: 0.1,
            ci_high: 0.5,
          },
        ]}
        xLabel="Relative time"
        yLabel="Coefficient"
        ariaLabel="Event study"
      />,
    );

    expect(host.querySelectorAll('[data-chart-mark="did-coefficient"]')).toHaveLength(1);
    expect(host.querySelector("svg")?.getAttribute("width")).toBe("480");
  });

  it("renders a unit circle and one keyboard-accessible point per VAR eigenvalue", () => {
    const data: VarStabilityPoint[] = [
      { re: 1.1, im: 0, modulus: 1.1, status: "stable" },
      { re: 0.5, im: -0.5, modulus: 0.7, status: "unstable" },
    ];

    renderChart(
      <VarStabilityChart
        data={data}
        xLabel="实部"
        yLabel="虚部"
        ariaLabel="特征根稳定性图"
        getPointLabel={(index) =>
          index === 0 ? "<img src=x onerror=alert(1)>" : `特征根 ${index + 1}`
        }
        getPointAriaLabel={(point, index) => `特征根 ${index + 1}，状态 ${point.status}`}
        modulusLabel="模"
        unstableTooltipLabel="<em>不稳定</em>"
        formatValue={(value, field) => `${field}:${value.toFixed(2)}`}
      />,
    );

    const svg = host.querySelector("svg");
    expect.soft(svg?.getAttribute("role")).toBe("group");
    expect.soft(svg?.getAttribute("aria-label")).toBe("特征根稳定性图");
    expect.soft(host.textContent).toContain("实部");
    expect.soft(host.textContent).toContain("虚部");

    const unitCircle = host.querySelector<SVGCircleElement>('[data-chart-reference="unit-circle"]');
    expect.soft(unitCircle).not.toBeNull();
    expect.soft(Number(unitCircle?.getAttribute("data-chart-value"))).toBe(1);

    const points = [
      ...host.querySelectorAll<SVGCircleElement>('[data-chart-mark="var-eigenvalue"]'),
    ];
    expect.soft(points).toHaveLength(data.length);
    expect.soft(points.map((point) => point.getAttribute("tabindex"))).toEqual(["0", "0"]);
    expect
      .soft(points.map((point) => point.getAttribute("aria-label")))
      .toEqual(["特征根 1，状态 stable", "特征根 2，状态 unstable"]);
    expect
      .soft(points.map((point) => point.getAttribute("data-status")))
      .toEqual(["stable", "unstable"]);
    expect
      .soft(points.map((point) => point.getAttribute("fill")))
      .toEqual([chartTheme.series.primary, chartTheme.series.negative]);

    act(() => points[0]?.dispatchEvent(new FocusEvent("focus")));
    const tooltip = host.querySelector<HTMLDivElement>("svg + div");
    expect.soft(tooltip?.style.opacity).toBe("1");
    expect.soft(tooltip?.textContent).toContain("<img src=x onerror=alert(1)>");
    expect.soft(tooltip?.querySelector("img")).toBeNull();
    expect.soft(tooltip?.textContent).toContain("modulus:1.10");

    act(() => points[1]?.dispatchEvent(new FocusEvent("focus")));
    expect.soft(tooltip?.textContent).toContain("<em>不稳定</em>");
    expect.soft(tooltip?.querySelector("em")).toBeNull();
  });
});
