import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  type WorkbenchGroupInfo,
  type WorkbenchEditorPanelInfo,
} from "@/modules/workbench/internal/dockview/workbenchRead";
import { workbenchDockviewRead } from "@/modules/workbench/internal/dockview/workbenchRead";
import { workbenchDockviewControl } from "@/modules/workbench/internal/dockview/workbenchControl";

const mocks = vi.hoisted(() => ({
  panels: [] as WorkbenchEditorPanelInfo[],
  groups: [] as WorkbenchGroupInfo[],
  openEditor: vi.fn(),
  ensureCentralGroup: vi.fn(async () => "central-group"),
  requestCloseEditorPanels: vi.fn(async () => true),
  showWorkbenchLayoutError: vi.fn(),
}));

vi.mock("@/modules/workbench/internal/application/workbenchLayoutErrorFeedback", () => ({
  showWorkbenchLayoutError: mocks.showWorkbenchLayoutError,
}));

vi.mock("@/features/core/graphSession/graphSessionStore", () => ({
  useGraphSessionStore: {
    getState: () => ({ focusedSession: null }),
  },
}));

vi.mock("./resolveResourceDisplayName", () => ({
  resolveResourceDisplayName: (_ref: unknown, fallback: string) => fallback,
}));

vi.mock("./rightSidebarActions", () => ({
  revealDetails: vi.fn(async () => undefined),
}));

vi.mock("./editorPanelCloseCommands", () => ({
  requestCloseEditorPanels: mocks.requestCloseEditorPanels,
}));

import { openEditorPanel } from "./openEditorPanel";

function editorPanel(
  panelInstanceId: string,
  groupId: string,
  resourceRef: string,
  metadata: { pinned?: boolean; sticky?: boolean } = {},
): WorkbenchEditorPanelInfo {
  return {
    panelInstanceId,
    groupId,
    component: "EditorResource",
    title: resourceRef,
    metadata: {
      role: "editor",
      resourceRef,
      resourceKind: "event",
      ...metadata,
    },
    active: true,
    location: { type: "grid" },
  };
}

function group(groupId: string, panelInstanceIds: readonly string[]): WorkbenchGroupInfo {
  return {
    groupId,
    panelInstanceIds,
    activePanelInstanceId: panelInstanceIds[0],
    active: true,
    location: { type: "grid" },
  };
}

describe("openEditorPanel", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    mocks.panels = [];
    mocks.groups = [];
    mocks.openEditor.mockReset();
    mocks.ensureCentralGroup.mockReset();
    mocks.ensureCentralGroup.mockResolvedValue("central-group");
    mocks.requestCloseEditorPanels.mockReset();
    mocks.requestCloseEditorPanels.mockResolvedValue(true);
    mocks.showWorkbenchLayoutError.mockReset();

    vi.spyOn(workbenchDockviewRead, "listGroups").mockImplementation(() => mocks.groups);
    vi.spyOn(workbenchDockviewRead, "listEditorPanelsInGroup").mockImplementation((groupId) =>
      mocks.panels.flatMap((panel) =>
        panel.groupId === groupId && panel.metadata.role === "editor"
          ? [{ ...panel, metadata: panel.metadata }]
          : [],
      ),
    );
    vi.spyOn(workbenchDockviewRead, "findEditorPanelsByResource").mockImplementation(
      (resourceRef) =>
        mocks.panels.filter(
          (panel) => panel.metadata.role === "editor" && panel.metadata.resourceRef === resourceRef,
        ),
    );
    vi.spyOn(workbenchDockviewRead, "getActiveEditorPanel").mockImplementation(() => {
      const active = mocks.panels.find((panel) => panel.active);
      return active?.metadata.role === "editor" ? active : undefined;
    });
    vi.spyOn(workbenchDockviewControl, "ensureCentralGroup").mockImplementation(
      mocks.ensureCentralGroup,
    );
    vi.spyOn(workbenchDockviewControl, "openEditor").mockImplementation(mocks.openEditor);
  });

  it("always opens editor resources as fixed tabs without preview replacement", async () => {
    const existing = editorPanel("panel-preview", "group-main", "events/Preview.yssbi-event", {
      pinned: false,
    });
    mocks.panels = [existing];
    mocks.groups = [group(existing.groupId, [existing.panelInstanceId])];
    const opened = editorPanel("panel-main", "group-main", "events/Main.yssbi-event", {
      pinned: true,
    });
    mocks.openEditor.mockResolvedValue(opened);

    await expect(
      openEditorPanel(
        { resourceRef: "events/Main.yssbi-event", resourceKind: "event" },
        { targetGroupId: "group-main" },
      ),
    ).resolves.toBe(opened);

    expect(mocks.openEditor).toHaveBeenCalledWith(
      expect.objectContaining({
        resourceRef: "events/Main.yssbi-event",
        pinned: true,
        mode: "reuse-resource",
      }),
    );
    expect(mocks.requestCloseEditorPanels).not.toHaveBeenCalled();
  });
});
