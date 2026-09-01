import { beforeEach, describe, expect, it, vi } from "vitest";

import type { WorkbenchPanelInfo } from "@/modules/workbench/internal/dockview/workbenchRead";
import { useEditorStore } from "@/features/core/editor";

const mocks = vi.hoisted(() => ({
  panels: [] as WorkbenchPanelInfo[],
  ensureView: vi.fn(),
  reveal: vi.fn(),
}));

vi.mock("i18next", () => ({
  default: { t: (key: string) => key },
}));

vi.mock("@/modules/workbench/internal/dockview/workbenchRead", () => ({
  workbenchDockviewRead: {
    listPanels: () => mocks.panels,
  },
}));

vi.mock("@/modules/workbench/internal/dockview/workbenchControl", () => ({
  workbenchDockviewControl: {
    ensureView: mocks.ensureView,
    reveal: mocks.reveal,
  },
}));

vi.mock("@/modules/workbench/internal/dockview/workbenchDockviewInternal", () => ({
  workbenchDockviewInternal: { runLayoutTransaction: vi.fn() },
}));

vi.mock("@/modules/workbench/internal/dockview/logsControl", () => ({
  logsDockviewControl: { resetToDefault: vi.fn() },
}));

vi.mock("@/modules/workbench/internal/application/workbenchLayoutController", () => ({
  workbenchLayoutController: {
    beginLayoutReset: vi.fn(),
    completeLayoutReset: vi.fn(),
  },
}));

vi.mock("@/features/application/editor/workbenchPanelClose", () => ({
  requestCloseWorkbenchPanel: vi.fn(),
}));

vi.mock("@/modules/workbench/internal/application/workbenchLayoutErrorFeedback", () => ({
  showWorkbenchLayoutError: vi.fn(),
}));

import {
  revealDetails,
  revealInspect,
  setDetailContext,
  setPassiveDetailContext,
  setInspectionContext,
} from "./rightSidebarActions";

function viewPanel(viewId: "details" | "inspect"): WorkbenchPanelInfo {
  return {
    panelInstanceId: `view:${viewId}`,
    groupId: "edge-right",
    component: viewId === "details" ? "Details" : "Inspect",
    title: viewId,
    metadata: { role: "view", viewId },
    active: true,
    location: { type: "edge", position: "right" },
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((next) => {
    resolve = next;
  });
  return { promise, resolve };
}

beforeEach(() => {
  vi.clearAllMocks();
  mocks.panels.splice(0);
  mocks.reveal.mockResolvedValue(true);
  mocks.ensureView.mockResolvedValue(viewPanel("details"));
  useEditorStore.setState({
    detailFocus: null,
    variablesGraphScopePath: null,
  });
});

describe("right sidebar context actions", () => {
  it("updates passive Details and Inspect context without ensuring or activating a view", () => {
    setDetailContext({ kind: "function", path: "functions/F.yssbi-function" });
    expect(useEditorStore.getState().detailFocus).toEqual({
      kind: "function",
      path: "functions/F.yssbi-function",
    });
    expect(useEditorStore.getState().variablesGraphScopePath).toBe("functions/F.yssbi-function");

    setInspectionContext("events/Main.yssbi-event", ["node-1"]);
    expect(useEditorStore.getState().detailFocus).toEqual({
      kind: "node",
      id: "node-1",
      graphPath: "events/Main.yssbi-event",
    });

    setInspectionContext("events/Main.yssbi-event", ["node-1", "node-2"]);
    expect(useEditorStore.getState().detailFocus).toBeNull();
    setDetailContext(null);

    expect(mocks.ensureView).not.toHaveBeenCalled();
    expect(mocks.reveal).not.toHaveBeenCalled();
  });

  it("keeps explicit node focus when passive graph hydration reports the same tab", () => {
    setInspectionContext("events/Main.yssbi-event", ["node-1"]);

    setPassiveDetailContext({ kind: "event", path: "events/Main.yssbi-event" });

    expect(useEditorStore.getState().detailFocus).toEqual({
      kind: "node",
      id: "node-1",
      graphPath: "events/Main.yssbi-event",
    });
  });

  it("updates explicit Details context without ensuring or activating a view", async () => {
    await revealDetails({ kind: "variable", id: "variable-1" });
    expect(useEditorStore.getState().detailFocus).toEqual({
      kind: "variable",
      id: "variable-1",
    });
    expect(mocks.ensureView).not.toHaveBeenCalled();
    expect(mocks.reveal).not.toHaveBeenCalled();
  });

  it("ensures Inspect after publishing its node context", async () => {
    const inspect = deferred<WorkbenchPanelInfo>();
    mocks.ensureView.mockImplementation(() => inspect.promise);
    let inspectSettled = false;
    const inspectReveal = revealInspect("events/Main.yssbi-event", ["node-2"]).then(() => {
      inspectSettled = true;
    });

    expect(useEditorStore.getState().detailFocus).toEqual({
      kind: "node",
      id: "node-2",
      graphPath: "events/Main.yssbi-event",
    });
    expect(mocks.ensureView).toHaveBeenLastCalledWith({
      viewId: "inspect",
      title: "panel.inspect",
    });
    await Promise.resolve();
    expect(inspectSettled).toBe(false);
    inspect.resolve(viewPanel("inspect"));
    await inspectReveal;

    expect(mocks.ensureView).toHaveBeenCalledOnce();
  });

  it("does not create Inspect for an invalid selection or erase non-node Details context", async () => {
    setDetailContext({ kind: "data", id: "database-1" });

    await revealInspect("events/Main.yssbi-event", []);

    expect(useEditorStore.getState().detailFocus).toEqual({
      kind: "data",
      id: "database-1",
    });
    expect(mocks.ensureView).not.toHaveBeenCalled();
    expect(mocks.reveal).not.toHaveBeenCalled();
  });
});
