// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { ChartThemeContextProvider, type ChartThemeValue } from "@/shared/charts/core/theme";
import type { ChartModel } from "@/shared/types/visualization/chartModel";
import { LinePlotControls } from "./LinePlotControls";

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

const lineModel: Extract<ChartModel, { kind: "line" }> = {
  kind: "line",
  points: [{ x: 1, y: 2 }],
  xAxis: { label: "Time", valueType: "number" },
  yAxis: { label: "Value", valueType: "number" },
  showPoints: true,
};

class TestResizeObserver implements ResizeObserver {
  readonly observe = vi.fn();
  readonly unobserve = vi.fn();
  readonly disconnect = vi.fn();

  constructor(_callback: ResizeObserverCallback) {}
}

let host: HTMLDivElement;
let root: Root;

beforeEach(() => {
  vi.stubGlobal("ResizeObserver", TestResizeObserver);
  vi.stubGlobal("requestAnimationFrame", (_callback: FrameRequestCallback) => 1);
  vi.stubGlobal("cancelAnimationFrame", vi.fn());
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

describe("LinePlotControls", () => {
  it("associates each label with a unique Switch ID across instances", () => {
    act(() => {
      root.render(
        <TooltipProvider>
          <ChartThemeContextProvider value={chartTheme}>
            <LinePlotControls model={lineModel} />
            <LinePlotControls model={lineModel} />
          </ChartThemeContextProvider>
        </TooltipProvider>,
      );
    });

    const toolbarButtons = [...host.querySelectorAll<HTMLButtonElement>("button")];
    expect(toolbarButtons).toHaveLength(2);

    act(() => toolbarButtons.forEach((button) => button.click()));

    const labels = [...host.querySelectorAll<HTMLLabelElement>("label")].filter(
      (label) => label.textContent === "Scatter Points",
    );
    const switchIds = labels.map((label) => label.htmlFor);

    expect(labels).toHaveLength(2);
    expect(switchIds.every(Boolean)).toBe(true);
    expect(new Set(switchIds)).toHaveLength(2);
    for (const switchId of switchIds) {
      const control = document.getElementById(switchId);
      expect(control?.getAttribute("data-slot")).toBe("switch");
      expect(host.contains(control)).toBe(true);
    }
  });
});
