import { describe, expect, it } from "vitest";
import {
  buildStatusBarItems,
  clearStatusBarRegistryForTests,
  createBuiltInStatusBarItems,
  registerStatusBarItem,
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

describe("statusBarRegistry", () => {
  it("merges built-in and registered items by alignment and priority", () => {
    clearStatusBarRegistryForTests();
    const unregister = registerStatusBarItem({
      id: "extension-item",
      alignment: "right",
      priority: 5,
      render: () => "ext",
    });

    const builtIn = createBuiltInStatusBarItems(noopActions);
    const snapshot = buildStatusBarItems(ctx, builtIn);

    expect(snapshot.left.map((item) => item.id)).toEqual([
      "project-status",
      "project-file",
      "active-tab",
    ]);
    expect(snapshot.right[0]?.id).toBe("extension-item");
    expect(snapshot.right.some((item) => item.id === "theme-mode")).toBe(true);

    unregister();
    clearStatusBarRegistryForTests();
  });

  it("honors visible predicate for registered items", () => {
    clearStatusBarRegistryForTests();
    registerStatusBarItem({
      id: "hidden-item",
      alignment: "left",
      priority: 1,
      visible: () => false,
      render: () => "hidden",
    });

    const snapshot = buildStatusBarItems(ctx, createBuiltInStatusBarItems(noopActions));
    expect(snapshot.left.some((item) => item.id === "hidden-item")).toBe(false);

    clearStatusBarRegistryForTests();
  });

  it("provides accessible names for interactive built-in items", () => {
    clearStatusBarRegistryForTests();
    const snapshot = buildStatusBarItems(ctx, createBuiltInStatusBarItems(noopActions));

    for (const item of [...snapshot.left, ...snapshot.right]) {
      if (!item.onClick) continue;
      expect(item.ariaLabel, item.id).toBeTruthy();
    }

    clearStatusBarRegistryForTests();
  });
});
