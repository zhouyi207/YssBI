import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  type WorkbenchGroupInfo,
  type WorkbenchEditorPanelInfo,
} from "@/features/core/dockview/workbenchRead";
import { workbenchDockviewRead } from "@/features/core/dockview/workbenchRead";
import { workbenchDockviewControl } from "@/features/core/dockview/workbenchControl";

const mocks = vi.hoisted(() => ({
  panels: [] as WorkbenchEditorPanelInfo[],
  groups: [] as WorkbenchGroupInfo[],
  openEditor: vi.fn(),
  ensureCentralGroup: vi.fn(async () => "central-group"),
  requestCloseEditorPanels: vi.fn(async () => true),
  showWorkbenchLayoutError: vi.fn(),
}));

vi.mock("@/features/application/layout/workbenchLayoutErrorFeedback", () => ({
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

  it("preserves an existing pinned and sticky editor during a preview open", async () => {
    const existing = editorPanel("panel-main", "group-main", "events/Main.yssbi-event", {
      pinned: true,
      sticky: true,
    });
    mocks.panels = [existing];
    mocks.groups = [group(existing.groupId, [existing.panelInstanceId])];
    mocks.openEditor.mockResolvedValue(existing);

    await openEditorPanel(
      {
        resourceRef: "events/Main.yssbi-event",
        resourceKind: "event",
        pinned: false,
      },
      {
        targetGroupId: "group-main",
      },
    );

    expect(mocks.openEditor).toHaveBeenCalledWith(
      expect.objectContaining({
        resourceRef: "events/Main.yssbi-event",
        pinned: true,
        sticky: true,
        mode: "reuse-resource",
      }),
    );
    expect(mocks.requestCloseEditorPanels).not.toHaveBeenCalled();
  });

  it("re-resolves to an ensured central group after closing the sole preview group", async () => {
    const preview = editorPanel("panel-preview", "group-preview", "events/Preview.yssbi-event", {
      pinned: false,
    });
    const opened = editorPanel("panel-next", "central-group", "events/Next.yssbi-event", {
      pinned: false,
    });
    mocks.panels = [preview];
    mocks.groups = [group(preview.groupId, [preview.panelInstanceId])];
    mocks.requestCloseEditorPanels.mockImplementationOnce(async () => {
      mocks.panels = [];
      mocks.groups = [];
      return true;
    });
    mocks.openEditor.mockResolvedValue(opened);

    await expect(
      openEditorPanel(
        {
          resourceRef: "events/Next.yssbi-event",
          resourceKind: "event",
          pinned: false,
        },
        {
          targetGroupId: preview.groupId,
        },
      ),
    ).resolves.toBe(opened);

    expect(mocks.requestCloseEditorPanels).toHaveBeenCalledWith([preview.panelInstanceId]);
    expect(mocks.ensureCentralGroup).toHaveBeenCalledOnce();
    expect(mocks.openEditor).toHaveBeenCalledWith(
      expect.objectContaining({
        resourceRef: "events/Next.yssbi-event",
        targetGroupId: "central-group",
        pinned: false,
      }),
    );
  });
});
