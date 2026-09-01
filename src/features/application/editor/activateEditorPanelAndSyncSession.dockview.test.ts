import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  WorkbenchEditorPanelInfo,
  WorkbenchGroupInfo,
} from "@/features/core/dockview/workbenchRead";

const mocks = vi.hoisted(() => ({
  activate: vi.fn(async () => true),
  groups: [] as WorkbenchGroupInfo[],
  panels: [] as WorkbenchEditorPanelInfo[],
  rootActivePanelInstanceId: null as string | null,
  getFocusedGroupId: vi.fn(),
  clearFocusedSession: vi.fn(),
  setDetailContext: vi.fn(),
}));

vi.mock("@/features/core/dockview/workbenchRead", () => ({
  workbenchDockviewRead: {
    listGroups: () => mocks.groups,
    listEditorPanelsInGroup: (groupId: string) =>
      mocks.panels.filter((panel) => panel.groupId === groupId),
    findEditorPanelsByResource: (resourceRef: string) =>
      mocks.panels.filter(
        (panel) => panel.metadata.role === "editor" && panel.metadata.resourceRef === resourceRef,
      ),
    getActivePanel: () =>
      mocks.panels.find((panel) => panel.panelInstanceId === mocks.rootActivePanelInstanceId),
    getActiveEditorPanel: () => mocks.panels.find((panel) => panel.active),
    getActiveEditorPanelInGroup: (groupId: string) => {
      const activePanelInstanceId = mocks.groups.find(
        (group) => group.groupId === groupId,
      )?.activePanelInstanceId;
      return mocks.panels.find((panel) => panel.panelInstanceId === activePanelInstanceId);
    },
    getPanel: (panelInstanceId: string) =>
      mocks.panels.find((panel) => panel.panelInstanceId === panelInstanceId),
  },
}));

vi.mock("@/features/core/dockview/workbenchControl", () => ({
  workbenchDockviewControl: {
    activate: mocks.activate,
  },
}));
vi.mock("@/features/core/graphSession/graphSessionStore", () => ({
  useGraphSessionStore: {
    getState: () => ({
      getFocusedGroupId: mocks.getFocusedGroupId,
      clearFocusedSession: mocks.clearFocusedSession,
    }),
  },
}));
vi.mock("./graphSessionLifecycle", () => ({
  suspendEditorGroupGraphSession: vi.fn(async () => undefined),
}));
vi.mock("./graphPanelSession", () => ({
  activateGraphPanelSession: vi.fn(async () => true),
}));
vi.mock("@/features/core/editor/detail/variablesGraphScope", () => ({
  syncVariablesGraphScopeFromActiveTab: vi.fn(),
}));
vi.mock("./rightSidebarActions", () => ({
  detailFocusForEditorResource: (
    resourceKind: "event" | "function" | "worksheet",
    resourceRef: string,
  ) =>
    resourceKind === "worksheet"
      ? { kind: "worksheet", worksheetPath: resourceRef }
      : { kind: resourceKind, path: resourceRef },
  setPassiveDetailContext: mocks.setDetailContext,
}));

import {
  activateCurrentEditorPanel,
  activateEditorPanelAndSyncSession,
  focusEditorGroupSync,
  synchronizeActiveEditorPanel,
} from "./activateEditorPanelAndSyncSession";

function editorPanel(
  panelInstanceId: string,
  resourceRef: string,
  active = false,
  resourceKind: "event" | "function" | "worksheet" = "event",
): WorkbenchEditorPanelInfo {
  return {
    panelInstanceId,
    groupId: "group-a",
    component: resourceKind === "worksheet" ? "WorksheetEditor" : "GraphEditor",
    title: resourceRef,
    metadata: { role: "editor", resourceRef, resourceKind },
    active,
    location: { type: "grid" },
  };
}

function group(activePanelInstanceId = "panel-a"): WorkbenchGroupInfo {
  return {
    groupId: "group-a",
    panelInstanceIds: mocks.panels.map((panel) => panel.panelInstanceId),
    activePanelInstanceId,
    active: true,
    location: { type: "grid" },
  };
}

describe("editor panel Dockview synchronization", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.getFocusedGroupId.mockReturnValue("group-a");
    mocks.panels = [editorPanel("panel-a", "events/A", true)];
    mocks.groups = [group()];
    mocks.rootActivePanelInstanceId = "panel-a";
  });

  it("does not write to Dockview while passively focusing its already active group", () => {
    expect(focusEditorGroupSync("group-a")).toBe(false);
    expect(mocks.activate).not.toHaveBeenCalled();
  });

  it("synchronizes a Dockview activation with passive detail context only", async () => {
    await synchronizeActiveEditorPanel(mocks.panels[0]);

    expect(mocks.activate).not.toHaveBeenCalled();
    expect(mocks.setDetailContext).toHaveBeenCalledWith({
      kind: "event",
      path: "events/A",
    });
  });

  it("physically activates an editor that is only active inside its inactive group", async () => {
    mocks.rootActivePanelInstanceId = null;

    await expect(activateEditorPanelAndSyncSession(mocks.panels[0])).resolves.toBe(true);

    expect(mocks.activate).toHaveBeenCalledOnce();
    expect(mocks.activate).toHaveBeenCalledWith("panel-a");
  });

  it("lets only the latest rapid application switch activate a canonical editor panel", async () => {
    mocks.panels = [editorPanel("panel-a", "events/A"), editorPanel("panel-b", "events/B")];
    mocks.groups = [group("panel-a")];

    const first = activateEditorPanelAndSyncSession(mocks.panels[0]);
    const second = activateEditorPanelAndSyncSession(mocks.panels[1]);
    await Promise.all([first, second]);

    expect(mocks.activate).toHaveBeenCalledTimes(1);
    expect(mocks.activate).toHaveBeenCalledWith("panel-b");
    expect(mocks.setDetailContext).toHaveBeenLastCalledWith({
      kind: "event",
      path: "events/B",
    });
  });

  it("hydrates a restored worksheet with passive context and no Dockview write-back", async () => {
    mocks.panels = [
      editorPanel("worksheet-panel", "worksheets/Summary.yssbi-worksheet", true, "worksheet"),
    ];
    mocks.groups = [group("worksheet-panel")];

    await expect(activateCurrentEditorPanel("group-a")).resolves.toBe(true);

    expect(mocks.activate).not.toHaveBeenCalled();
    expect(mocks.setDetailContext).toHaveBeenCalledWith({
      kind: "worksheet",
      worksheetPath: "worksheets/Summary.yssbi-worksheet",
    });
    expect(mocks.clearFocusedSession).toHaveBeenCalledWith("group-a");
  });
});
