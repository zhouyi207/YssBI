// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ChartThemeContextProvider, type ChartThemeValue } from "@/shared/charts/core/theme";
import { CorrelationMatrixChart } from "./CorrelationMatrixChart";

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

const labels = "abcdefghijklmnopqrst".split("");
const matrix = labels.map((_, row) =>
  labels.map((__, column) => {
    if (row === 0 && column === 1) return 0.5;
    if (row === labels.length - 1 && column === labels.length - 1) return 1;
    return null;
  }),
);
const pMatrix = labels.map((_, row) =>
  labels.map((__, column) => {
    if (row === 0 && column === 1) return 0.04;
    if (row === labels.length - 1 && column === labels.length - 1) return 0;
    return null;
  }),
);

let host: HTMLDivElement;
let root: Root;
let pendingFrames: Map<number, FrameRequestCallback>;
let nextFrameId: number;

function flushMeasurement(): void {
  const frames = [...pendingFrames.values()];
  pendingFrames.clear();
  act(() => frames.forEach((callback) => callback(0)));
}

function translatedLeftEdge(mark: SVGRectElement): number {
  let translatedX = 0;
  let ancestor = mark.parentElement;
  while (ancestor && ancestor.tagName.toLowerCase() !== "svg") {
    const transform = ancestor.getAttribute("transform") ?? "";
    translatedX += Number(/^translate\(([-\d.]+)/.exec(transform)?.[1] ?? 0);
    ancestor = ancestor.parentElement;
  }
  return translatedX + Number(mark.getAttribute("x"));
}

function translatedRightEdge(mark: SVGRectElement): number {
  return translatedLeftEdge(mark) + Number(mark.getAttribute("width"));
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
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(250);
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(250);
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

describe("CorrelationMatrixChart", () => {
  it("renders only available cells accessibly within a narrow chart box", () => {
    act(() => {
      root.render(
        <ChartThemeContextProvider value={chartTheme}>
          <CorrelationMatrixChart labels={labels} matrix={matrix} pMatrix={pMatrix} height={250} />
        </ChartThemeContextProvider>,
      );
    });
    flushMeasurement();

    const svg = host.querySelector("svg");
    expect.soft(svg?.getAttribute("role")).toBe("group");
    expect.soft(svg?.getAttribute("aria-label")).toBe("Correlation matrix");

    const cells = [...host.querySelectorAll<SVGRectElement>("rect.cell")];
    expect.soft(cells).toHaveLength(2);

    const firstCell = cells[0];
    expect.soft(firstCell?.getAttribute("tabindex")).toBe("0");
    expect.soft(firstCell?.getAttribute("aria-label")).toContain("a by b");
    firstCell?.dispatchEvent(new FocusEvent("focus"));
    const tooltip = host.querySelector("svg + div");
    expect.soft(tooltip?.textContent).toContain("Row: a");
    expect.soft(tooltip?.textContent).toContain("Column: b");
    expect.soft(tooltip?.textContent).toContain("0.500");
    expect.soft(tooltip?.textContent).toContain("p = 0.040");

    const legend = host.querySelector<SVGRectElement>('[data-chart-legend="correlation-scale"]');
    expect.soft(legend).not.toBeNull();
    if (legend) {
      expect
        .soft(translatedRightEdge(legend))
        .toBeLessThanOrEqual(Math.min(...cells.map(translatedLeftEdge)));
    }

    const svgWidth = Number(host.querySelector("svg")?.getAttribute("width"));
    expect.soft(Math.max(...cells.map(translatedRightEdge))).toBeLessThanOrEqual(svgWidth);
  });
});
