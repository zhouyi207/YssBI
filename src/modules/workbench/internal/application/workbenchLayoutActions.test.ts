import type { SerializedDockview } from "dockview-react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { WorkbenchLayoutTransaction } from "../dockview/workbenchDockviewInternal";
import type {
  ConfigureWorkbenchEdgeRequest,
  EnsureViewRequest,
  MoveWorkbenchPanelRequest,
  WorkbenchEdgePosition,
  WorkbenchEdgeState,
  WorkbenchGroupInfo,
  WorkbenchPanelInfo,
} from "../dockview/workbenchTypes";
import type { WorkbenchPanelMetadata, WorkbenchViewId } from "../dockview/workbenchPanelModel";
import { useEditorStore } from "@/features/core/editor";
import { useGraphSessionStore } from "@/features/core/graphSession/graphSessionStore";

const mocks = vi.hoisted(() => ({
  panels: [] as WorkbenchPanelInfo[],
  bottomEdge: {
    position: "bottom",
    exists: false,
    visible: false,
    collapsed: false,
  } as WorkbenchEdgeState,
  leftEdge: {
    position: "left",
    exists: false,
    visible: false,
    collapsed: false,
  } as WorkbenchEdgeState,
  ensureView: vi.fn(),
  reveal: vi.fn(),
  setEdgeCollapsed: vi.fn(),
  requestCloseWorkbenchPanel: vi.fn(),
  runLayoutTransaction: vi.fn(),
  beginLayoutReset: vi.fn(),
  completeLayoutReset: vi.fn(),
  resetLogs: vi.fn(),
  showWorkbenchLayoutError: vi.fn(),
  transaction: null as WorkbenchLayoutTransaction | null,
}));

vi.mock("i18next", () => ({
  default: { t: (key: string) => key },
}));

vi.mock("../dockview/workbenchRead", () => ({
  workbenchDockviewRead: {
    listPanels: () => mocks.panels,
    listGroupPanels: (groupId: string) => mocks.panels.filter((panel) => panel.groupId === groupId),
    getEdgeState: (position: WorkbenchEdgePosition) =>
      position === "left" ? { ...mocks.leftEdge } : { ...mocks.bottomEdge },
  },
}));

vi.mock("../dockview/workbenchControl", () => ({
  workbenchDockviewControl: {
    ensureView: mocks.ensureView,
    reveal: mocks.reveal,
    setEdgeCollapsed: mocks.setEdgeCollapsed,
  },
}));

vi.mock("../dockview/workbenchDockviewInternal", () => ({
  workbenchDockviewInternal: {
    runLayoutTransaction: mocks.runLayoutTransaction,
  },
}));

vi.mock("../dockview/logsControl", () => ({
  logsDockviewControl: {
    resetToDefault: mocks.resetLogs,
  },
}));

vi.mock("./workbenchLayoutController", () => ({
  workbenchLayoutController: {
    beginLayoutReset: mocks.beginLayoutReset,
    completeLayoutReset: mocks.completeLayoutReset,
  },
}));

vi.mock("./panelCommands", () => ({
  closeWorkbenchViewPanel: mocks.requestCloseWorkbenchPanel,
}));

vi.mock("./workbenchLayoutErrorFeedback", () => ({
  showWorkbenchLayoutError: mocks.showWorkbenchLayoutError,
}));

import * as layoutActions from "./workbenchLayoutActions";

const {
  resetWorkbenchLayout,
  revealWorkbenchView,
  toggleActivityWorkbenchGroup,
  toggleBottomWorkbenchGroup,
  toggleWorkbenchView,
} = layoutActions;

type GroupSeed = {
  readonly groupId: string;
  readonly panelInstanceIds: readonly string[];
  readonly activePanelInstanceId?: string;
  readonly active?: boolean;
  readonly collapsed?: boolean;
  readonly visible?: boolean;
  readonly location: WorkbenchGroupInfo["location"];
};

type MutableGroup = {
  groupId: string;
  panelInstanceIds: string[];
  activePanelInstanceId?: string;
  active: boolean;
  location: WorkbenchGroupInfo["location"];
};

type MutablePanel = {
  panelInstanceId: string;
  groupId: string;
  component: WorkbenchPanelInfo["component"];
  title?: string;
  metadata: WorkbenchPanelMetadata;
  active: boolean;
  location: WorkbenchPanelInfo["location"];
};

const edgeLocation = (position: WorkbenchEdgePosition) => ({
  type: "edge" as const,
  position,
});
const gridLocation = { type: "grid" as const };

function componentFor(metadata: WorkbenchPanelMetadata): WorkbenchPanelInfo["component"] {
  if (metadata.role === "editor") {
    return "EditorResource";
  }
  if (metadata.role === "result") return "Result";
  return (
    {
      project: "Project",
      nodes: "Nodes",
      data: "Data",
      commands: "Commands",
      details: "Details",
      assistant: "Assistant",
      inspect: "Inspect",
      logs: "Logs",
      output: "Output",
      diagnostics: "Diagnostics",
    } as const
  )[metadata.viewId];
}

function panel(
  panelInstanceId: string,
  groupId: string,
  metadata: WorkbenchPanelMetadata,
  location: WorkbenchPanelInfo["location"],
  active = false,
): WorkbenchPanelInfo {
  return {
    panelInstanceId,
    groupId,
    component: componentFor(metadata),
    title:
      metadata.role === "view"
        ? metadata.viewId
        : metadata.role === "result"
          ? metadata.title
          : metadata.resourceRef,
    metadata,
    active,
    location,
  };
}

function viewPanel(
  panelInstanceId: string,
  viewId: WorkbenchViewId,
  groupId = `edge-${viewId}`,
  location: WorkbenchPanelInfo["location"] = edgeLocation(
    ["project", "nodes", "data", "commands"].includes(viewId)
      ? "left"
      : viewId === "logs" || viewId === "output" || viewId === "diagnostics"
        ? "bottom"
        : "right",
  ),
  active = false,
): WorkbenchPanelInfo {
  return panel(panelInstanceId, groupId, { role: "view", viewId }, location, active);
}

function editorPanel(
  panelInstanceId: string,
  resourceRef: string,
  groupId: string,
  location: WorkbenchPanelInfo["location"],
  active = false,
): WorkbenchPanelInfo {
  return panel(
    panelInstanceId,
    groupId,
    { role: "editor", resourceRef, resourceKind: "event" },
    location,
    active,
  );
}

function resultPanel(
  panelInstanceId: string,
  resultKey: string,
  groupId: string,
  location: WorkbenchPanelInfo["location"],
  active = false,
): WorkbenchPanelInfo {
  return panel(
    panelInstanceId,
    groupId,
    {
      role: "result",
      resultKey,
      resultId: `${resultKey}-payload`,
      title: resultKey,
      presentation: { kind: "inspector" },
      source: null,
    },
    location,
    active,
  );
}

function serializedLayout(groups: readonly GroupSeed[]): SerializedDockview {
  const gridLeaves = groups
    .filter((group) => group.location.type === "grid")
    .map((group) => ({
      type: "leaf" as const,
      data: {
        id: group.groupId,
        views: [...group.panelInstanceIds],
        ...(group.activePanelInstanceId ? { activeView: group.activePanelInstanceId } : {}),
      },
    }));
  const edgeGroups = Object.fromEntries(
    groups.flatMap((group) =>
      group.location.type === "edge"
        ? [
            [
              group.location.position,
              {
                size: 200,
                visible: group.visible ?? true,
                collapsed: group.collapsed ?? false,
                group: {
                  id: group.groupId,
                  views: [...group.panelInstanceIds],
                  ...(group.activePanelInstanceId
                    ? { activeView: group.activePanelInstanceId }
                    : {}),
                },
              },
            ],
          ]
        : [],
    ),
  );
  const ids = groups.flatMap((group) => group.panelInstanceIds);
  return {
    grid: {
      root: { type: "branch", data: gridLeaves },
      height: 800,
      width: 1200,
      orientation: "HORIZONTAL",
    },
    panels: Object.fromEntries(ids.map((id) => [id, { id }])),
    edgeGroups,
    floatingGroups: [],
    popoutGroups: [],
  } as unknown as SerializedDockview;
}

function nestedGridLayout(groups: readonly GroupSeed[]): SerializedDockview {
  const layout = serializedLayout(groups) as unknown as {
    grid: { root: { type: "branch"; data: unknown[] } };
  };
  const gridGroups = groups.filter((group) => group.location.type === "grid");
  const leaf = (group: GroupSeed) => ({
    type: "leaf",
    data: {
      id: group.groupId,
      views: [...group.panelInstanceIds],
      ...(group.activePanelInstanceId ? { activeView: group.activePanelInstanceId } : {}),
    },
  });
  layout.grid.root.data =
    gridGroups.length < 2
      ? gridGroups.map(leaf)
      : [leaf(gridGroups[0]), { type: "branch", data: gridGroups.slice(1).map(leaf) }];
  return layout as unknown as SerializedDockview;
}

function createTransactionHarness(
  initialPanels: readonly WorkbenchPanelInfo[],
  seeds: readonly GroupSeed[],
  serialized = serializedLayout(seeds),
) {
  const panelOrder = initialPanels.map((candidate) => candidate.panelInstanceId);
  const panels = new Map<string, MutablePanel>(
    initialPanels.map((candidate) => [
      candidate.panelInstanceId,
      {
        ...candidate,
        metadata: structuredClone(candidate.metadata),
        location: { ...candidate.location },
      },
    ]),
  );
  const groupOrder = seeds.map((seed) => seed.groupId);
  const groups = new Map<string, MutableGroup>(
    seeds.map((seed) => [
      seed.groupId,
      {
        groupId: seed.groupId,
        panelInstanceIds: [...seed.panelInstanceIds],
        activePanelInstanceId: seed.activePanelInstanceId,
        active: seed.active ?? false,
        location: { ...seed.location },
      },
    ]),
  );
  const moveCalls: Array<MoveWorkbenchPanelRequest & { success: boolean }> = [];
  const configureCalls: ConfigureWorkbenchEdgeRequest[] = [];
  const ensureCalls: EnsureViewRequest[] = [];
  const activateCalls: string[] = [];
  const removeCalls: string[][] = [];
  let generatedId = 0;

  const info = (candidate: MutablePanel): WorkbenchPanelInfo => ({
    panelInstanceId: candidate.panelInstanceId,
    groupId: candidate.groupId,
    component: candidate.component,
    ...(candidate.title === undefined ? {} : { title: candidate.title }),
    metadata: structuredClone(candidate.metadata),
    active: candidate.active,
    location: { ...candidate.location },
  });

  const setActive = (panelInstanceId: string): boolean => {
    const target = panels.get(panelInstanceId);
    if (!target) return false;
    for (const candidate of panels.values()) candidate.active = false;
    for (const group of groups.values()) group.active = false;
    target.active = true;
    const group = groups.get(target.groupId);
    if (group) {
      group.active = true;
      group.activePanelInstanceId = panelInstanceId;
    }
    activateCalls.push(panelInstanceId);
    return true;
  };

  const ensureEdge = (position: WorkbenchEdgePosition): MutableGroup => {
    const existing = groupOrder
      .map((groupId) => groups.get(groupId))
      .find((group) => group?.location.type === "edge" && group.location.position === position);
    if (existing) return existing;
    const created: MutableGroup = {
      groupId: `edge-${position}`,
      panelInstanceIds: [],
      active: false,
      location: edgeLocation(position),
    };
    groups.set(created.groupId, created);
    groupOrder.push(created.groupId);
    return created;
  };

  const ensureView = (request: EnsureViewRequest): WorkbenchPanelInfo => {
    ensureCalls.push(request);
    const existing = [...panels.values()].find(
      (candidate) =>
        candidate.metadata.role === "view" && candidate.metadata.viewId === request.viewId,
    );
    if (existing) {
      existing.title = request.title;
      setActive(existing.panelInstanceId);
      return info(existing);
    }

    const position = ["project", "nodes", "data", "commands"].includes(request.viewId)
      ? "left"
      : request.viewId === "logs" || request.viewId === "output" || request.viewId === "diagnostics"
        ? "bottom"
        : "right";
    const group = ensureEdge(position);
    generatedId += 1;
    const panelInstanceId = `created:${request.viewId}:${generatedId}`;
    const metadata: WorkbenchPanelMetadata = { role: "view", viewId: request.viewId };
    const created: MutablePanel = {
      panelInstanceId,
      groupId: group.groupId,
      component: componentFor(metadata),
      title: request.title,
      metadata,
      active: false,
      location: { ...group.location },
    };
    panels.set(panelInstanceId, created);
    panelOrder.push(panelInstanceId);
    group.panelInstanceIds.push(panelInstanceId);
    group.activePanelInstanceId ??= panelInstanceId;
    setActive(panelInstanceId);
    return info(created);
  };

  const tx: WorkbenchLayoutTransaction = {
    serialize: () => structuredClone(serialized),
    getPanel: (panelInstanceId) => {
      const candidate = panels.get(panelInstanceId);
      return candidate ? info(candidate) : undefined;
    },
    getActivePanel: () => {
      const candidate = [...panels.values()].find((panelState) => panelState.active);
      return candidate ? info(candidate) : undefined;
    },
    listPanels: () =>
      panelOrder.flatMap((panelInstanceId) => {
        const candidate = panels.get(panelInstanceId);
        return candidate ? [info(candidate)] : [];
      }),
    listGroups: () =>
      groupOrder.flatMap((groupId) => {
        const group = groups.get(groupId);
        return group
          ? [
              {
                groupId,
                panelInstanceIds: [...group.panelInstanceIds],
                ...(group.activePanelInstanceId
                  ? { activePanelInstanceId: group.activePanelInstanceId }
                  : {}),
                active: group.active,
                location: { ...group.location },
              },
            ]
          : [];
      }),
    listGroupPanels: (groupId) => {
      const group = groups.get(groupId);
      return (
        group?.panelInstanceIds.flatMap((panelInstanceId) => {
          const candidate = panels.get(panelInstanceId);
          return candidate ? [info(candidate)] : [];
        }) ?? []
      );
    },
    ensureCentralGroup: () => {
      const active = groupOrder
        .map((groupId) => groups.get(groupId))
        .find((group) => group?.active && group.location.type === "grid");
      if (active) return active.groupId;
      const existing = groupOrder
        .map((groupId) => groups.get(groupId))
        .find((group) => group?.location.type === "grid");
      if (existing) return existing.groupId;
      const groupId = "grid-created-central";
      groups.set(groupId, {
        groupId,
        panelInstanceIds: [],
        active: false,
        location: gridLocation,
      });
      groupOrder.push(groupId);
      return groupId;
    },
    ensureView,
    move: (request) => {
      const candidate = panels.get(request.panelInstanceId);
      const source = candidate ? groups.get(candidate.groupId) : undefined;
      const target = groups.get(request.groupId);
      if (!candidate || !source || !target) {
        moveCalls.push({ ...request, success: false });
        return false;
      }
      const currentIndex = source.panelInstanceIds.indexOf(candidate.panelInstanceId);
      const requestedIndex =
        request.index ?? (source === target ? currentIndex : target.panelInstanceIds.length);
      if (source === target && requestedIndex === currentIndex) {
        if (request.activate !== false) setActive(candidate.panelInstanceId);
        moveCalls.push({ ...request, success: true });
        return true;
      }

      source.panelInstanceIds.splice(currentIndex, 1);
      if (source.activePanelInstanceId === candidate.panelInstanceId) {
        source.activePanelInstanceId = source.panelInstanceIds[0];
      }
      if (
        source !== target &&
        source.location.type === "grid" &&
        source.panelInstanceIds.length === 0
      ) {
        groups.delete(source.groupId);
        groupOrder.splice(groupOrder.indexOf(source.groupId), 1);
      }
      const insertionIndex = Math.min(requestedIndex, target.panelInstanceIds.length);
      target.panelInstanceIds.splice(insertionIndex, 0, candidate.panelInstanceId);
      target.activePanelInstanceId ??= candidate.panelInstanceId;
      candidate.groupId = target.groupId;
      candidate.location = { ...target.location };
      if (request.activate !== false) setActive(candidate.panelInstanceId);
      moveCalls.push({ ...request, success: true });
      return true;
    },
    configureEdge: (request) => {
      configureCalls.push({ ...request });
      const group = ensureEdge(request.position);
      return {
        position: request.position,
        exists: true,
        groupId: group.groupId,
        visible: true,
        collapsed: request.collapsed,
        size: request.size,
      };
    },
    activate: setActive,
    removePanels: (panelInstanceIds) => {
      removeCalls.push([...panelInstanceIds]);
    },
  };

  return {
    tx,
    moveCalls,
    configureCalls,
    ensureCalls,
    activateCalls,
    removeCalls,
    panelIds: () => panelOrder.filter((panelInstanceId) => panels.has(panelInstanceId)),
    groupPanelIds: (groupId: string) => [...(groups.get(groupId)?.panelInstanceIds ?? [])],
    activePanelId: () =>
      [...panels.values()].find((candidate) => candidate.active)?.panelInstanceId,
    hasGroup: (groupId: string) => groups.has(groupId),
  };
}

beforeEach(() => {
  for (const mock of [
    mocks.ensureView,
    mocks.reveal,
    mocks.setEdgeCollapsed,
    mocks.requestCloseWorkbenchPanel,
    mocks.runLayoutTransaction,
    mocks.beginLayoutReset,
    mocks.completeLayoutReset,
    mocks.resetLogs,
    mocks.showWorkbenchLayoutError,
  ]) {
    mock.mockReset();
  }
  mocks.panels.splice(0);
  mocks.bottomEdge = {
    position: "bottom",
    exists: false,
    visible: false,
    collapsed: false,
  };
  mocks.leftEdge = {
    position: "left",
    exists: false,
    visible: false,
    collapsed: false,
  };
  mocks.transaction = null;
  mocks.ensureView.mockImplementation(async ({ viewId }: EnsureViewRequest) =>
    viewPanel(`created:${viewId}`, viewId),
  );
  mocks.reveal.mockResolvedValue(true);
  mocks.setEdgeCollapsed.mockImplementation(async (position: string, collapsed: boolean) => {
    if (position === "left") mocks.leftEdge = { ...mocks.leftEdge, collapsed };
    else mocks.bottomEdge = { ...mocks.bottomEdge, collapsed };
    return true;
  });
  mocks.requestCloseWorkbenchPanel.mockResolvedValue(true);
  mocks.beginLayoutReset.mockReturnValue(41);
  mocks.runLayoutTransaction.mockImplementation(
    async (operation: (transaction: WorkbenchLayoutTransaction) => unknown) => {
      if (!mocks.transaction) throw new Error("missing test transaction");
      return operation(mocks.transaction);
    },
  );
  useEditorStore.setState({ detailFocus: null, variablesGraphScopePath: null });
  useGraphSessionStore.getState().reset();
});

describe("semantic workbench layout actions", () => {
  it("exports only the four planned actions and reveals an existing view in place", async () => {
    expect(Object.keys(layoutActions).sort()).toEqual([
      "resetWorkbenchLayout",
      "revealWorkbenchView",
      "toggleActivityWorkbenchGroup",
      "toggleBottomWorkbenchGroup",
      "toggleWorkbenchView",
    ]);
    const logs = viewPanel("logs-moved", "logs", "edge-right", edgeLocation("right"));
    mocks.panels.push(logs);

    await expect(revealWorkbenchView("logs")).resolves.toEqual(logs);

    expect(mocks.reveal).toHaveBeenCalledWith("logs-moved");
    expect(mocks.ensureView).not.toHaveBeenCalled();
  });

  it("expands an existing but hidden Activity edge instead of collapsing it", async () => {
    mocks.leftEdge = {
      position: "left",
      exists: true,
      groupId: "workbench-edge-left",
      visible: false,
      collapsed: false,
      size: 292,
    };
    mocks.panels.push(
      viewPanel("project", "project", "workbench-edge-left", edgeLocation("left")),
      viewPanel("nodes", "nodes", "workbench-edge-left", edgeLocation("left")),
      viewPanel("data", "data", "workbench-edge-left", edgeLocation("left")),
      viewPanel("commands", "commands", "workbench-edge-left", edgeLocation("left")),
    );

    await toggleActivityWorkbenchGroup();

    expect(mocks.setEdgeCollapsed).toHaveBeenCalledWith("left", false);
    expect(mocks.runLayoutTransaction).not.toHaveBeenCalled();
  });

  it("creates a missing default tool through its deterministic home request", async () => {
    const created = viewPanel("logs-created", "logs", "edge-bottom", edgeLocation("bottom"));
    mocks.ensureView.mockResolvedValueOnce(created);

    await expect(revealWorkbenchView("logs")).resolves.toEqual(created);

    expect(mocks.ensureView).toHaveBeenCalledOnce();
    expect(mocks.ensureView).toHaveBeenCalledWith({
      viewId: "logs",
      title: "panel.logs",
    });
  });

  it("creates permanent Details without context but still requires node context for Inspect", async () => {
    const createdDetails = viewPanel(
      "details-created",
      "details",
      "edge-right",
      edgeLocation("right"),
    );
    mocks.ensureView.mockResolvedValueOnce(createdDetails);
    await expect(revealWorkbenchView("details")).resolves.toEqual(createdDetails);
    await expect(revealWorkbenchView("inspect")).resolves.toBeNull();
    expect(mocks.ensureView).toHaveBeenCalledWith({
      viewId: "details",
      title: "panel.details",
    });

    const openDetails = viewPanel("details-open", "details", "edge-right", edgeLocation("right"));
    mocks.panels.push(openDetails);
    await expect(revealWorkbenchView("details")).resolves.toEqual(openDetails);
    expect(mocks.reveal).toHaveBeenCalledWith("details-open");

    mocks.panels.splice(0);
    useEditorStore.setState({ detailFocus: { kind: "variable", id: "variable-1" } });
    await revealWorkbenchView("details");
    useEditorStore.setState({
      detailFocus: {
        kind: "node",
        id: "node-1",
        graphPath: "events/Main.yssbi-event",
      },
    });
    await revealWorkbenchView("inspect");

    expect(mocks.ensureView.mock.calls.map(([request]) => request.viewId)).toEqual([
      "details",
      "details",
      "inspect",
    ]);
  });

  it("closes an existing singleton only through the batch close coordinator", async () => {
    mocks.panels.push(viewPanel("output-open", "output"));

    await expect(toggleWorkbenchView("output")).resolves.toBe(true);

    expect(mocks.requestCloseWorkbenchPanel).toHaveBeenCalledOnce();
    expect(mocks.requestCloseWorkbenchPanel).toHaveBeenCalledWith("output-open");
    expect(mocks.ensureView).not.toHaveBeenCalled();
    expect(mocks.reveal).not.toHaveBeenCalled();
  });

  it("collapses and expands a non-empty native bottom edge group", async () => {
    mocks.bottomEdge = {
      position: "bottom",
      exists: true,
      groupId: "edge-bottom",
      visible: true,
      collapsed: false,
      size: 200,
    };
    mocks.panels.push(viewPanel("output-bottom", "output", "edge-bottom"));

    await toggleBottomWorkbenchGroup();
    await toggleBottomWorkbenchGroup();

    expect(mocks.setEdgeCollapsed.mock.calls).toEqual([
      ["bottom", true],
      ["bottom", false],
    ]);
    expect(mocks.reveal).not.toHaveBeenCalled();
    expect(mocks.ensureView).not.toHaveBeenCalled();
  });

  it("reveals moved Logs when bottom is empty and creates Logs only when absent", async () => {
    mocks.bottomEdge = {
      position: "bottom",
      exists: true,
      groupId: "edge-bottom",
      visible: true,
      collapsed: false,
      size: 200,
    };
    mocks.panels.push(viewPanel("logs-grid", "logs", "grid-a", gridLocation));

    await toggleBottomWorkbenchGroup();

    expect(mocks.reveal).toHaveBeenCalledWith("logs-grid");
    expect(mocks.ensureView).not.toHaveBeenCalled();

    mocks.panels.splice(0);
    mocks.reveal.mockClear();
    await toggleBottomWorkbenchGroup();

    expect(mocks.ensureView).toHaveBeenCalledWith({
      viewId: "logs",
      title: "panel.logs",
    });
  });

  it("routes each ensure or reveal rejection through typed feedback once", async () => {
    const ensureFailure = new Error("ensure failed");
    mocks.ensureView.mockRejectedValueOnce(ensureFailure);
    await expect(revealWorkbenchView("logs")).resolves.toBeNull();
    expect(mocks.showWorkbenchLayoutError).toHaveBeenLastCalledWith(ensureFailure);

    const revealFailure = new Error("reveal failed");
    mocks.panels.push(viewPanel("logs-open", "logs"));
    mocks.reveal.mockRejectedValueOnce(revealFailure);
    await expect(revealWorkbenchView("logs")).resolves.toBeNull();
    expect(mocks.showWorkbenchLayoutError).toHaveBeenLastCalledWith(revealFailure);
    expect(mocks.showWorkbenchLayoutError).toHaveBeenCalledTimes(2);
  });
});

describe("resetWorkbenchLayout", () => {
  it("preserves editor and Result identities while restoring deterministic homes, order, sizes, and active editor", async () => {
    const groups: GroupSeed[] = [
      {
        groupId: "edge-left",
        panelInstanceIds: [
          "project",
          "nodes",
          "data",
          "commands",
          "output",
          "editor-left-a",
          "editor-left-b",
        ],
        collapsed: true,
        location: edgeLocation("left"),
      },
      {
        groupId: "edge-top",
        panelInstanceIds: ["editor-top"],
        collapsed: true,
        location: edgeLocation("top"),
      },
      {
        groupId: "grid-a",
        panelInstanceIds: ["result-grid", "editor-grid-a", "editor-grid-b"],
        activePanelInstanceId: "editor-grid-b",
        active: true,
        location: gridLocation,
      },
      {
        groupId: "grid-b",
        panelInstanceIds: ["logs", "editor-grid-c"],
        location: gridLocation,
      },
      {
        groupId: "edge-right",
        panelInstanceIds: ["details", "editor-right", "result-right", "inspect"],
        collapsed: true,
        location: edgeLocation("right"),
      },
      {
        groupId: "edge-bottom",
        panelInstanceIds: ["editor-bottom-a", "editor-bottom-b"],
        collapsed: true,
        location: edgeLocation("bottom"),
      },
    ];
    const initial = [
      viewPanel("project", "project", "edge-left", edgeLocation("left")),
      viewPanel("nodes", "nodes", "edge-left", edgeLocation("left")),
      viewPanel("data", "data", "edge-left", edgeLocation("left")),
      viewPanel("commands", "commands", "edge-left", edgeLocation("left")),
      viewPanel("output", "output", "edge-left", edgeLocation("left")),
      editorPanel("editor-left-a", "events/LeftA.yssbi-event", "edge-left", edgeLocation("left")),
      editorPanel("editor-left-b", "events/LeftB.yssbi-event", "edge-left", edgeLocation("left")),
      editorPanel("editor-top", "events/Top.yssbi-event", "edge-top", edgeLocation("top")),
      resultPanel("result-grid", "result-grid", "grid-a", gridLocation),
      editorPanel("editor-grid-a", "events/GridA.yssbi-event", "grid-a", gridLocation),
      editorPanel("editor-grid-b", "events/GridB.yssbi-event", "grid-a", gridLocation, true),
      viewPanel("logs", "logs", "grid-b", gridLocation),
      editorPanel("editor-grid-c", "events/GridC.yssbi-event", "grid-b", gridLocation),
      viewPanel("details", "details", "edge-right", edgeLocation("right")),
      editorPanel("editor-right", "events/Right.yssbi-event", "edge-right", edgeLocation("right")),
      resultPanel("result-right", "result-right", "edge-right", edgeLocation("right")),
      viewPanel("inspect", "inspect", "edge-right", edgeLocation("right")),
      editorPanel(
        "editor-bottom-a",
        "events/BottomA.yssbi-event",
        "edge-bottom",
        edgeLocation("bottom"),
      ),
      editorPanel(
        "editor-bottom-b",
        "events/BottomB.yssbi-event",
        "edge-bottom",
        edgeLocation("bottom"),
      ),
    ];
    const beforeIds = initial.map((candidate) => candidate.panelInstanceId).sort();
    const harness = createTransactionHarness(initial, groups, nestedGridLayout(groups));
    mocks.transaction = harness.tx;
    useGraphSessionStore.setState({
      focusedSession: { groupId: "edge-left", graphPath: "events/LeftA.yssbi-event" },
    });

    await resetWorkbenchLayout();

    expect(harness.panelIds().sort()).toEqual(
      [...beforeIds, "created:assistant:1", "created:diagnostics:2"].sort(),
    );
    expect(harness.groupPanelIds("grid-a")).toEqual([
      "editor-left-a",
      "editor-left-b",
      "editor-top",
      "editor-grid-a",
      "editor-grid-b",
      "editor-grid-c",
      "editor-right",
      "editor-bottom-a",
      "editor-bottom-b",
    ]);
    expect(harness.groupPanelIds("edge-left")).toEqual(["project", "data", "nodes", "commands"]);
    expect(harness.groupPanelIds("edge-bottom")).toEqual([
      "logs",
      "output",
      "created:diagnostics:2",
    ]);
    expect(harness.groupPanelIds("edge-right")).toEqual([
      "details",
      "created:assistant:1",
      "result-grid",
      "result-right",
      "inspect",
    ]);
    expect(harness.configureCalls).toEqual([
      { position: "left", size: 292, collapsed: false, headerPosition: "left" },
      { position: "right", size: 320, collapsed: false, headerPosition: "right" },
      { position: "bottom", size: 200, collapsed: false, headerPosition: "bottom" },
    ]);
    expect(harness.activePanelId()).toBe("editor-grid-b");
    expect(harness.removeCalls).toEqual([]);
    expect(mocks.requestCloseWorkbenchPanel).not.toHaveBeenCalled();
    expect(mocks.resetLogs).toHaveBeenCalledOnce();
    expect(mocks.beginLayoutReset).toHaveBeenCalledOnce();
    expect(mocks.completeLayoutReset).toHaveBeenCalledWith(41);
  });

  it("keeps the only central target alive by moving the first edge editor before Activity leaves", async () => {
    const groups: GroupSeed[] = [
      {
        groupId: "grid-only",
        panelInstanceIds: ["project"],
        activePanelInstanceId: "project",
        active: true,
        location: gridLocation,
      },
      {
        groupId: "edge-left",
        panelInstanceIds: ["editor-left"],
        location: edgeLocation("left"),
      },
      {
        groupId: "edge-top",
        panelInstanceIds: ["editor-top"],
        location: edgeLocation("top"),
      },
    ];
    const initial = [
      viewPanel("project", "project", "grid-only", gridLocation, true),
      editorPanel("editor-left", "events/Left.yssbi-event", "edge-left", edgeLocation("left")),
      editorPanel("editor-top", "events/Top.yssbi-event", "edge-top", edgeLocation("top")),
    ];
    const harness = createTransactionHarness(initial, groups);
    mocks.transaction = harness.tx;

    await resetWorkbenchLayout();

    const firstEditorMove = harness.moveCalls.findIndex(
      (request) => request.panelInstanceId === "editor-left",
    );
    const activityMove = harness.moveCalls.findIndex(
      (request) => request.panelInstanceId === "project",
    );
    expect(firstEditorMove).toBeGreaterThanOrEqual(0);
    expect(firstEditorMove).toBeLessThan(activityMove);
    expect(harness.moveCalls.every((request) => request.success)).toBe(true);
    expect(harness.hasGroup("grid-only")).toBe(true);
    expect(harness.groupPanelIds("grid-only")).toEqual(["editor-left", "editor-top"]);
  });

  it("reactivates the validated recent editor when a Result is physically active", async () => {
    const groups: GroupSeed[] = [
      {
        groupId: "grid-a",
        panelInstanceIds: ["editor-a"],
        location: gridLocation,
      },
      {
        groupId: "edge-right",
        panelInstanceIds: ["result-active"],
        activePanelInstanceId: "result-active",
        active: true,
        location: edgeLocation("right"),
      },
    ];
    const initial = [
      editorPanel("editor-a", "events/Main.yssbi-event", "grid-a", gridLocation),
      resultPanel("result-active", "result-active", "edge-right", edgeLocation("right"), true),
    ];
    const harness = createTransactionHarness(initial, groups);
    mocks.transaction = harness.tx;
    useGraphSessionStore.setState({
      focusedSession: { groupId: "grid-a", graphPath: "events/Main.yssbi-event" },
    });

    await resetWorkbenchLayout();

    expect(harness.activePanelId()).toBe("editor-a");
    expect(harness.activateCalls[harness.activateCalls.length - 1]).toBe("editor-a");
  });

  it("falls back to the first deterministic editor when no session exists", async () => {
    const groups: GroupSeed[] = [
      {
        groupId: "edge-left",
        panelInstanceIds: ["editor-first"],
        location: edgeLocation("left"),
      },
      {
        groupId: "grid-a",
        panelInstanceIds: ["editor-grid"],
        location: gridLocation,
      },
      {
        groupId: "edge-right",
        panelInstanceIds: ["result-active"],
        activePanelInstanceId: "result-active",
        active: true,
        location: edgeLocation("right"),
      },
    ];
    const initial = [
      editorPanel("editor-first", "events/First.yssbi-event", "edge-left", edgeLocation("left")),
      editorPanel("editor-grid", "events/Grid.yssbi-event", "grid-a", gridLocation),
      resultPanel("result-active", "result-active", "edge-right", edgeLocation("right"), true),
    ];
    const harness = createTransactionHarness(initial, groups);
    mocks.transaction = harness.tx;

    await resetWorkbenchLayout();

    expect(harness.activePanelId()).toBe("editor-first");
  });

  it("activates Project only when the reset snapshot contains no editor", async () => {
    const groups: GroupSeed[] = [
      {
        groupId: "grid-only",
        panelInstanceIds: ["project", "nodes", "data", "commands"],
        activePanelInstanceId: "project",
        active: true,
        location: gridLocation,
      },
    ];
    const harness = createTransactionHarness(
      [
        viewPanel("project", "project", "grid-only", gridLocation, true),
        viewPanel("nodes", "nodes", "grid-only", gridLocation),
        viewPanel("data", "data", "grid-only", gridLocation),
        viewPanel("commands", "commands", "grid-only", gridLocation),
      ],
      groups,
    );
    mocks.transaction = harness.tx;

    await resetWorkbenchLayout();

    expect(harness.activePanelId()).toBe("project");
    expect(harness.activateCalls[harness.activateCalls.length - 1]).toBe("project");
    expect(harness.removeCalls).toEqual([]);
  });
});
