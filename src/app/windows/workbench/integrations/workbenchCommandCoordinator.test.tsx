// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { WorkbenchCommandCapability } from "@/features/application/editor";
import { useWorkbenchCommandCoordinator } from "./workbenchCommandCoordinator";

const mocks = vi.hoisted(() => ({
  openGraph: vi.fn(),
  editor: {
    undo: vi.fn(),
    redo: vi.fn(),
    copy: vi.fn(),
    cut: vi.fn(),
    paste: vi.fn(),
    deleteSelected: vi.fn(),
    duplicateSelected: vi.fn(),
  },
  canvas: {
    selectAllNodes: vi.fn(),
    focusSelectedNodes: vi.fn(),
    fitCompleteGraph: vi.fn(),
  },
  project: {
    saveGraph: vi.fn(),
    saveGraphAs: vi.fn(),
    importGraph: vi.fn(),
  },
  panels: {
    openGraph: vi.fn(),
    splitEditorRight: vi.fn(),
  },
  graphs: {
    addEvent: vi.fn(),
    addFunction: vi.fn(),
  },
  charts: {
    addChart: vi.fn(),
  },
}));

vi.mock("@/features/application/editor", () => ({
  useEditorOperations: () => mocks.editor,
  useGraphCanvasCommands: () => mocks.canvas,
  useProjectOperations: () => mocks.project,
  useEditorPanelCommands: () => mocks.panels,
  useOpenChart: () => vi.fn(),
  useChartManagement: () => mocks.charts,
}));

vi.mock("@/features/application/dataManagement", () => ({
  useGraphManagement: (openGraph: typeof mocks.openGraph) => {
    mocks.openGraph = openGraph;
    return mocks.graphs;
  },
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe("useWorkbenchCommandCoordinator", () => {
  let root: Root;
  let commands: WorkbenchCommandCapability;

  beforeEach(() => {
    vi.clearAllMocks();
    root = createRoot(document.createElement("div"));
    function Harness() {
      commands = useWorkbenchCommandCoordinator();
      return null;
    }
    act(() => root.render(<Harness />));
  });

  afterEach(() => {
    act(() => root.unmount());
  });

  it("exposes only the caller-shaped Workbench command surface", () => {
    expect(Object.keys(commands).sort()).toEqual(
      [
        "addChart",
        "addEvent",
        "addFunction",
        "copy",
        "cut",
        "deleteSelected",
        "duplicateSelected",
        "fitCompleteGraph",
        "focusSelectedNodes",
        "importGraph",
        "paste",
        "redo",
        "saveGraph",
        "saveGraphAs",
        "selectAllNodes",
        "splitEditorRight",
        "undo",
      ].sort(),
    );
    expect(mocks.openGraph).toBe(mocks.panels.openGraph);
  });
});
