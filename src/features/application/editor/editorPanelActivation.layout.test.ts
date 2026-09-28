import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  WorkbenchEditorPanelInfo,
  WorkbenchGroupInfo,
} from "@/modules/workbench/internal/layout/workbenchRead";
import { resolveEditorDetailFocus } from "@/features/core/editor/detail/editorDetailPolicy";
import { RESOURCE_KINDS, type ResourceKind } from "@/shared/types/domain/resource";

const mocks = vi.hoisted(() => ({
  groups: [] as WorkbenchGroupInfo[],
  panels: [] as WorkbenchEditorPanelInfo[],
  rootActivePanelInstanceId: null as string | null,
  getFocusedGroupId: vi.fn(),
  clearFocusedSession: vi.fn(),
  setDetailContext: vi.fn(),
  reveal: vi.fn(async () => null),
}));

vi.mock("@/modules/workbench/internal/layout/workbenchRead", () => ({
  workbenchLayoutRead: {
    isReady: true,
    getActiveEditorPanel: () =>
      mocks.panels.find((panel) => panel.panelInstanceId === mocks.rootActivePanelInstanceId),
    getActiveEditorPanelInGroup: (groupId: string) => {
      const activePanelInstanceId = mocks.groups.find(
        (group) => group.groupId === groupId,
      )?.activePanelInstanceId;
      return mocks.panels.find((panel) => panel.panelInstanceId === activePanelInstanceId);
    },
  },
}));

vi.mock("@/modules/workbench/internal/application/workbenchLayoutActions", () => ({
  revealWorkbenchView: mocks.reveal,
}));
vi.mock("@/features/core/graphSession/graphSessionStore", () => ({
  useGraphSessionStore: {
    getState: () => ({
      getFocusedGroupId: mocks.getFocusedGroupId,
      clearFocusedSession: mocks.clearFocusedSession,
    }),
  },
}));
vi.mock("./graphPanelSession", () => ({
  focusGraphPanelSession: vi.fn(),
}));
vi.mock("./rightSidebarActions", () => ({
  detailFocusForEditorResource: (
    resourceKind: ResourceKind,
    resourceRef: string,
    panelInstanceId: string,
  ) => resolveEditorDetailFocus({ resourceKind, resourceRef, panelInstanceId }, []),
  setDetailContext: mocks.setDetailContext,
}));

import {
  synchronizeCurrentEditorPanel,
  revealActiveEditorDetails,
  synchronizeActiveEditorPanel,
} from "./editorPanelActivation";

function editorPanel(
  panelInstanceId: string,
  resourceRef: string,
  active = false,
  resourceKind: ResourceKind = "event_graph",
): WorkbenchEditorPanelInfo {
  return {
    panelInstanceId,
    groupId: "group-a",
    component: "EditorResource",
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

describe("editor panel FlexLayout synchronization", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.getFocusedGroupId.mockReturnValue("group-a");
    mocks.panels = [editorPanel("panel-a", "events/A", true)];
    mocks.groups = [group()];
    mocks.rootActivePanelInstanceId = "panel-a";
  });

  it("synchronizes a FlexLayout activation with passive detail context only", () => {
    synchronizeActiveEditorPanel(mocks.panels[0]);

    expect(mocks.reveal).not.toHaveBeenCalled();
    expect(mocks.setDetailContext).toHaveBeenCalledWith({
      kind: "event_graph",
      path: "events/A",
    });
  });

  it("does not publish an editor that is only selected inside an inactive group", async () => {
    mocks.rootActivePanelInstanceId = null;

    await expect(revealActiveEditorDetails(mocks.panels[0])).resolves.toBe(false);

    expect(mocks.rootActivePanelInstanceId).toBeNull();
    expect(mocks.setDetailContext).not.toHaveBeenCalled();
    expect(mocks.reveal).not.toHaveBeenCalled();
  });

  it("keeps the native selection when an earlier open result finishes late", async () => {
    mocks.panels = [editorPanel("panel-a", "events/A"), editorPanel("panel-b", "events/B")];
    mocks.groups = [group("panel-b")];
    mocks.rootActivePanelInstanceId = "panel-b";

    await expect(revealActiveEditorDetails(mocks.panels[0])).resolves.toBe(false);
    await expect(revealActiveEditorDetails(mocks.panels[1])).resolves.toBe(true);

    expect(mocks.rootActivePanelInstanceId).toBe("panel-b");
    expect(mocks.reveal).toHaveBeenCalledExactlyOnceWith("details");
    expect(mocks.setDetailContext).toHaveBeenLastCalledWith({
      kind: "event_graph",
      path: "events/B",
    });
  });

  it("synchronizes a restored chart with passive context and no FlexLayout write-back", () => {
    mocks.panels = [editorPanel("chart-panel", "charts/Summary.yssbi-chart", true, "chart")];
    mocks.groups = [group("chart-panel")];
    mocks.rootActivePanelInstanceId = "chart-panel";

    expect(synchronizeCurrentEditorPanel("group-a")).toBe(true);

    expect(mocks.reveal).not.toHaveBeenCalled();
    expect(mocks.setDetailContext).toHaveBeenCalledWith({
      kind: "chart",
      chartPath: "charts/Summary.yssbi-chart",
    });
    expect(mocks.clearFocusedSession).toHaveBeenCalledWith("group-a");
  });

  it("does not overwrite a native tab switch while Details reveal is settling", async () => {
    const panel = mocks.panels[0];
    mocks.panels.push(editorPanel("panel-b", "events/B"));
    let complete!: (value: null) => void;
    mocks.reveal.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          complete = resolve;
        }),
    );
    const activation = revealActiveEditorDetails(panel);
    await Promise.resolve();
    mocks.rootActivePanelInstanceId = "panel-b";
    synchronizeActiveEditorPanel(mocks.panels[1]);
    mocks.setDetailContext.mockClear();
    complete(null);

    await expect(activation).resolves.toBe(false);
    expect(mocks.setDetailContext).not.toHaveBeenCalled();
    expect(mocks.reveal).toHaveBeenCalledExactlyOnceWith("details");
    expect(synchronizeActiveEditorPanel(panel)).toBe(false);
  });

  it("rejects a captured panel whose resource identity changed", async () => {
    const captured = mocks.panels[0];
    mocks.panels = [editorPanel(captured.panelInstanceId, "events/A", true, "function_graph")];

    expect(synchronizeActiveEditorPanel(captured)).toBe(false);
    await expect(revealActiveEditorDetails(captured)).resolves.toBe(false);
    expect(mocks.setDetailContext).not.toHaveBeenCalled();
    expect(mocks.reveal).not.toHaveBeenCalled();
  });

  it("reveals Details after synchronizing every explicitly opened resource kind", async () => {
    for (const resourceKind of RESOURCE_KINDS) {
      mocks.reveal.mockClear();
      const panel = editorPanel(
        `panel-${resourceKind}`,
        `resource-${resourceKind}`,
        true,
        resourceKind,
      );
      mocks.panels = [panel];
      mocks.groups = [group(panel.panelInstanceId)];
      mocks.rootActivePanelInstanceId = panel.panelInstanceId;
      await expect(revealActiveEditorDetails(panel)).resolves.toBe(true);
      expect(mocks.reveal).toHaveBeenCalledExactlyOnceWith("details");
      expect(mocks.setDetailContext).toHaveBeenLastCalledWith(
        resolveEditorDetailFocus(
          {
            resourceKind,
            resourceRef: panel.metadata.resourceRef,
            panelInstanceId: panel.panelInstanceId,
          },
          [],
        ),
      );
      expect(mocks.setDetailContext.mock.invocationCallOrder.slice(-1)[0]).toBeLessThan(
        mocks.reveal.mock.invocationCallOrder[0],
      );
    }
  });
});
