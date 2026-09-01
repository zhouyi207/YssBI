import { beforeEach, describe, expect, it, vi } from "vitest";

import type { WorkbenchPanelInfo } from "@/features/core/dockview/workbenchRead";

const mocks = vi.hoisted(() => ({
  panels: [] as WorkbenchPanelInfo[],
  requestCloseWorkbenchPanels: vi.fn(async () => true),
}));

vi.mock("@/features/core/dockview/workbenchRead", () => ({
  workbenchDockviewRead: {
    listEditorPanelsInGroup: (groupId: string) =>
      mocks.panels.flatMap((panel) =>
        panel.groupId === groupId && panel.metadata.role === "editor"
          ? [{ ...panel, metadata: panel.metadata }]
          : [],
      ),
    getActiveEditorPanel: () => undefined,
  },
}));

vi.mock("@/features/core/editor/stores/useEditorStore", () => ({
  useEditorStore: {
    getState: () => ({ detailFocus: { kind: "event", path: "events/Main.yssbi-event" } }),
  },
}));

vi.mock("@/features/core/resource", () => ({
  isResourceDocumentDirty: vi.fn(() => false),
}));

vi.mock("./editorGroupCommands", () => ({
  splitEditorPanel: vi.fn(async () => undefined),
}));

vi.mock("./rightSidebarActions", () => ({
  detailFocusForEditorResource: (
    resourceKind: "event" | "function" | "worksheet",
    resourceRef: string,
  ) =>
    resourceKind === "worksheet"
      ? { kind: "worksheet", worksheetPath: resourceRef }
      : { kind: resourceKind, path: resourceRef },
  setDetailContext: vi.fn(),
}));

vi.mock("./workbenchPanelClose", () => ({
  requestCloseWorkbenchPanels: mocks.requestCloseWorkbenchPanels,
}));

import { requestCloseEditorPanel, requestCloseOtherEditorPanels } from "./editorPanelCloseCommands";

function editorPanel(panelInstanceId: string, resourceRef: string): WorkbenchPanelInfo {
  return {
    panelInstanceId,
    groupId: "group-a",
    component: "EditorResource",
    title: resourceRef,
    metadata: {
      role: "editor",
      resourceRef,
      resourceKind: "event",
    },
    active: panelInstanceId === "panel-a",
    location: { type: "grid" },
  };
}

describe("editor panel close commands", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.panels = [];
    mocks.requestCloseWorkbenchPanels.mockResolvedValue(true);
  });

  it("closes duplicate same-resource instances other than the exact kept panel", async () => {
    mocks.panels = [
      editorPanel("panel-a", "events/Main.yssbi-event"),
      editorPanel("panel-b", "events/Main.yssbi-event"),
      editorPanel("panel-c", "events/Other.yssbi-event"),
    ];

    await expect(requestCloseOtherEditorPanels("group-a", "panel-a")).resolves.toBe(true);

    expect(mocks.requestCloseWorkbenchPanels).toHaveBeenCalledWith(["panel-b", "panel-c"]);
  });

  it("closes the exact physical tab when same-resource instances share a group", async () => {
    mocks.panels = [
      editorPanel("panel-a", "events/Main.yssbi-event"),
      editorPanel("panel-b", "events/Main.yssbi-event"),
    ];

    await expect(requestCloseEditorPanel("panel-b")).resolves.toBe(true);

    expect(mocks.requestCloseWorkbenchPanels).toHaveBeenCalledWith(["panel-b"]);
  });
});
