import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  WorkbenchGroupInfo,
  WorkbenchPanelInfo,
} from "@/modules/workbench/internal/dockview/workbenchRead";
import { useGraphSessionStore } from "@/features/core/graphSession/graphSessionStore";

const mocks = vi.hoisted(() => ({
  panels: [] as WorkbenchPanelInfo[],
  groups: [] as WorkbenchGroupInfo[],
  activePanel: undefined as WorkbenchPanelInfo | undefined,
  ensureCentralGroup: vi.fn(async () => "central-group"),
}));

vi.mock("@/modules/workbench/internal/dockview/workbenchRead", () => ({
  workbenchDockviewRead: {
    listGroups: () => mocks.groups,
    listGroupPanels: (groupId: string) => mocks.panels.filter((panel) => panel.groupId === groupId),
    findEditorPanelsByResource: (resourceRef: string) =>
      mocks.panels.filter(
        (panel) => panel.metadata.role === "editor" && panel.metadata.resourceRef === resourceRef,
      ),
    getActivePanel: () => mocks.activePanel,
    getActiveEditorPanel: () =>
      mocks.activePanel?.metadata.role === "editor" ? mocks.activePanel : undefined,
  },
}));

vi.mock("@/modules/workbench/internal/dockview/workbenchControl", () => ({
  workbenchDockviewControl: {
    ensureCentralGroup: mocks.ensureCentralGroup,
  },
}));

import { resolveEditorOpenTargetGroupId } from "./editorOpenTarget";

function editorPanel(
  panelInstanceId: string,
  groupId: string,
  resourceRef: string,
): WorkbenchPanelInfo {
  return {
    panelInstanceId,
    groupId,
    component: "EditorResource",
    title: resourceRef,
    metadata: {
      role: "editor",
      resourceRef,
      resourceKind: "event",
    },
    active: false,
    location: { type: "grid" },
  };
}

function toolPanel(panelInstanceId: string, groupId: string): WorkbenchPanelInfo {
  return {
    panelInstanceId,
    groupId,
    component: "Logs",
    title: "Logs",
    metadata: { role: "view", viewId: "logs" },
    active: true,
    location: { type: "grid" },
  };
}

function group(
  groupId: string,
  panelInstanceIds: readonly string[],
  activePanelInstanceId?: string,
): WorkbenchGroupInfo {
  return {
    groupId,
    panelInstanceIds,
    ...(activePanelInstanceId ? { activePanelInstanceId } : {}),
    active: activePanelInstanceId !== undefined,
    location: { type: "grid" },
  };
}

beforeEach(() => {
  mocks.panels = [];
  mocks.groups = [];
  mocks.activePanel = undefined;
  mocks.ensureCentralGroup.mockReset();
  mocks.ensureCentralGroup.mockResolvedValue("central-group");
  useGraphSessionStore.getState().reset();
});

describe("resolveEditorOpenTargetGroupId", () => {
  it("uses a still-valid explicit group before editor context", async () => {
    const focused = editorPanel("editor-focused", "focused-group", "events/Focused.yssbi-event");
    mocks.panels = [focused];
    mocks.groups = [
      group("explicit-group", []),
      group("focused-group", [focused.panelInstanceId], focused.panelInstanceId),
    ];
    mocks.activePanel = focused;
    useGraphSessionStore
      .getState()
      .setFocusedSession("focused-group", "events/Focused.yssbi-event");

    await expect(resolveEditorOpenTargetGroupId("explicit-group")).resolves.toBe("explicit-group");
    expect(mocks.ensureCentralGroup).not.toHaveBeenCalled();
  });

  it("falls back from an invalid explicit group to the focused matching graph", async () => {
    const focused = editorPanel("editor-focused", "focused-group", "events/Focused.yssbi-event");
    const tool = toolPanel("logs", "tool-group");
    mocks.panels = [focused, tool];
    mocks.groups = [
      group("focused-group", [focused.panelInstanceId]),
      group("tool-group", [tool.panelInstanceId], tool.panelInstanceId),
    ];
    mocks.activePanel = tool;
    useGraphSessionStore
      .getState()
      .setFocusedSession("focused-group", "events/Focused.yssbi-event");

    await expect(resolveEditorOpenTargetGroupId("removed-group")).resolves.toBe("focused-group");
    expect(mocks.ensureCentralGroup).not.toHaveBeenCalled();
  });

  it("uses a live-validated recent editor group when its focused graph no longer matches", async () => {
    const recentEditor = editorPanel(
      "editor-recent",
      "recent-group",
      "events/StillOpen.yssbi-event",
    );
    const tool = toolPanel("logs", "recent-group");
    mocks.panels = [recentEditor, tool];
    mocks.groups = [
      group(
        "recent-group",
        [recentEditor.panelInstanceId, tool.panelInstanceId],
        tool.panelInstanceId,
      ),
    ];
    mocks.activePanel = tool;
    useGraphSessionStore.getState().setFocusedSession("recent-group", "events/Closed.yssbi-event");

    await expect(resolveEditorOpenTargetGroupId()).resolves.toBe("recent-group");
    expect(mocks.ensureCentralGroup).not.toHaveBeenCalled();
  });

  it("ensures a central group when no live editor context remains", async () => {
    const tool = toolPanel("logs", "tool-only-group");
    mocks.panels = [tool];
    mocks.groups = [group("tool-only-group", [tool.panelInstanceId], tool.panelInstanceId)];
    mocks.activePanel = tool;
    useGraphSessionStore
      .getState()
      .setFocusedSession("tool-only-group", "events/Closed.yssbi-event");

    await expect(resolveEditorOpenTargetGroupId()).resolves.toBe("central-group");
    expect(mocks.ensureCentralGroup).toHaveBeenCalledOnce();
  });
});
