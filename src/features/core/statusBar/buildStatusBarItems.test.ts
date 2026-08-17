import { describe, expect, it } from "vitest";
import {
  buildStatusBarItems,
  createBuiltInStatusBarItems,
} from "@/features/core/statusBar";
import type { StatusBarRenderContext } from "@/features/core/statusBar";

const ctx: StatusBarRenderContext = {
  t: ((key: string) => key) as StatusBarRenderContext["t"],
  projectStatus: "ready",
  projectFileName: "demo.yss",
  activeTitle: "Graph A",
  activeType: "event",
  activeTabId: "graph/a",
  activeEditorGroupId: "editor",
  selectedCount: 2,
  nodeCount: 5,
  connectionCount: 3,
  executionStatus: "idle",
  colorTheme: "Dark Modern (Default)",
  juliaWorkerState: "ready",
  juliaWorkerLabel: "Julia ready",
  juliaWorkerTooltip: "Julia worker is ready",
};

const noopActions = {
  openLogsPanel: () => {},
  resetCanvasViewport: () => {},
  cycleColorTheme: () => {},
  executionTooltip: "execution",
  themeTooltip: "theme",
  viewportTooltip: "viewport",
  renderViewportStatus: () => "X 0 Y 0 100%",
};

describe("built-in status bar items", () => {
  it("orders built-in items by alignment and priority", () => {
    const snapshot = buildStatusBarItems(ctx, createBuiltInStatusBarItems(noopActions));

    expect(snapshot.left.map((item) => item.id)).toEqual([
      "project-status",
      "project-file",
      "active-tab",
    ]);
    expect(snapshot.right.map((item) => item.id)).toEqual([
      "julia-worker",
      "node-count",
      "connection-count",
      "selected-nodes",
      "execution-status",
      "viewport-status",
      "theme-mode",
    ]);
  });

  it("provides accessible names for interactive built-in items", () => {
    const snapshot = buildStatusBarItems(ctx, createBuiltInStatusBarItems(noopActions));

    for (const item of [...snapshot.left, ...snapshot.right]) {
      if (!item.onClick) continue;
      expect(item.ariaLabel, item.id).toBeTruthy();
    }
  });
});
