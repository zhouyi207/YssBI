import {
  installGraphProjectionFixture,
  makeGraphEditingState,
  makeEditorProjectionFixture,
  makeGraphEditorSession,
} from "@/tests/helpers/editorProjectionFixtures";

import { beforeEach, describe, expect, it, vi } from "vitest";
import type { WorkbenchPanelInfo } from "@/modules/workbench/internal/layout/workbenchRead";
import { buildFileResourceMeta, resourceKey } from "@/features/core/resource";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import { getGraphSnapshot } from "@/features/core/graph/read";
import { projectSnapshotFixture } from "@/tests/helpers/projectSnapshotFixtures";

import {
  captureProjectIdentity,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import {
  prepareProjectSnapshotCommit,
  commitPreparedProjectSnapshot,
} from "./projectPublicationSnapshot";
import { commitEditorLayoutPublication } from "./editorLayoutPublicationCommit";

const flexlayoutMocks = vi.hoisted(() => {
  const panels: WorkbenchPanelInfo[] = [];
  const releasePane = vi.fn();
  const remapResource = vi.fn((from: string, to: string) => {
    for (let index = 0; index < panels.length; index += 1) {
      const panel = panels[index];
      if (panel.metadata.role !== "editor" || panel.metadata.resourceRef !== from) continue;
      panels[index] = {
        ...panel,
        metadata: { ...panel.metadata, resourceRef: to },
      };
    }
    return panels.filter(
      (panel) => panel.metadata.role === "editor" && panel.metadata.resourceRef === to,
    ).length;
  });
  const removePanels = vi.fn((panelInstanceIds: readonly string[]) => {
    const removed = new Set(panelInstanceIds);
    for (let index = panels.length - 1; index >= 0; index -= 1) {
      if (removed.has(panels[index].panelInstanceId)) panels.splice(index, 1);
    }
  });
  const transaction = {
    listPanels: () => panels,
    remapResource,
    removePanels,
  };
  const runPublicationTransaction = vi.fn(
    async (operation: (value: typeof transaction) => unknown | Promise<unknown>) =>
      operation(transaction),
  );

  return {
    panels,
    ready: true,
    releasePane,
    remapResource,
    removePanels,
    runPublicationTransaction,
  };
});

vi.mock("@/modules/workbench/internal/layout/workbenchRead", () => ({
  workbenchLayoutRead: {
    get isReady() {
      return flexlayoutMocks.ready;
    },
    getPanel: (id: string) => flexlayoutMocks.panels.find((panel) => panel.panelInstanceId === id),
  },
}));

vi.mock("@/modules/workbench/internal/layout/workbenchLayoutInternal", () => ({
  workbenchLayoutRuntime: { control: {} },
  workbenchLayoutInternal: {
    runPublicationTransaction: flexlayoutMocks.runPublicationTransaction,
  },
}));

vi.mock("@/modules/workbench/internal/layout/editorPaneStateStore", () => ({
  useEditorPaneStateStore: {
    getState: () => ({ release: flexlayoutMocks.releasePane }),
  },
}));

const caller = "events/Caller.yssbi-event";

function callerSnapshot() {
  return structuredClone(useResourceStore.getState().graphEntities[caller]);
}

describe("project snapshot projection replacement", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    flexlayoutMocks.ready = true;
    flexlayoutMocks.panels.splice(0);
    useResourceStore.getState().clear();
    const projection = makeEditorProjectionFixture({
      graphPath: caller,
      nodeId: "call-1",
      nodeTypeId: "yssbi.project.function.call",
      title: "Loaded caller",
    }).projection;
    installGraphProjectionFixture(caller, projection);
  });

  it("publishes function resources and signatures in the same project frame", async () => {
    startProjectLifecycle("project-1");
    const { plan } = projectSnapshotFixture(0);
    const path = "functions/Current.yssbi-function";
    plan.index.functionGraphs = [
      {
        path,
        name: "Current",
        type: "function_graph",
        revision: 2,
        functionRevision: 2,
        functionSignature: { parameters: [], return_type: "Text" },
        functionEditorProjection: { functionRevision: 2, inputs: [], outputs: [] },
      },
    ];
    const prepared = prepareProjectSnapshotCommit({ ...plan, ...captureProjectIdentity() });
    const observed: unknown[] = [];
    const key = resourceKey({ kind: "function_graph", id: path });
    const stop = useResourceStore.subscribe((state) =>
      observed.push({
        resourceRevision: state.resources[key]?.revision,
        functionRevision: getGraphSnapshot().graphMeta[path]?.functionRevision,
      }),
    );
    try {
      await commitPreparedProjectSnapshot(prepared);
      expect(observed).toEqual([{ resourceRevision: 2, functionRevision: 2 }]);
      useResourceStore.getState().clear();
      expect(observed).toEqual([
        { resourceRevision: 2, functionRevision: 2 },
        { resourceRevision: undefined, functionRevision: undefined },
      ]);
    } finally {
      stop();
    }
  });

  it("does not replace a dirty Graph draft, including edits made after snapshot preparation", async () => {
    const replacement = makeEditorProjectionFixture({ graphPath: caller });
    useResourceStore
      .getState()
      .installGraphSession(caller, makeGraphEditorSession(replacement.projection), {
        mode: "load",
      });
    const setDirty = (saveDirty: boolean) =>
      useResourceStore.setState((state) => ({
        sessions: { ...state.sessions, [caller]: { ...state.sessions[caller], saveDirty } },
      }));
    setDirty(true);
    startProjectLifecycle("project-a");
    const plan = prepareProjectSnapshotCommit({
      ...captureProjectIdentity(),
      publicationRevision: 2,
      activityPanels: [],
      index: {
        projectInstanceId: "project-a",
        projectName: "Project",
        exportTime: "",
        publicationRevision: 2,
        eventGraphs: [{ path: caller, name: "Caller", type: "event_graph", revision: 2 }],
        functionGraphs: [],
        minds: [],
        docs: [],
        charts: [],
        databases: [],
      },
      graphSessions: new Map([
        [
          caller,
          {
            resultState: makeGraphEditorSession(replacement.projection).resultState,

            document: makeGraphEditorSession(replacement.projection).document,
            editing: makeGraphEditingState(),
            projection: replacement.projection,
          },
        ],
      ]),
      chartDocuments: new Map(),
      pathRemaps: new Map(),
      filePathRemaps: new Map(),
      deletedResources: new Set(),
    });

    expect(plan.graphProjectionPlan.graphPaths).toEqual([]);
    const before = callerSnapshot();
    expect(plan.graphProjectionPlan.state.graphEntities[caller]).toEqual(before);
    setDirty(false);
    const preparedClean = prepareProjectSnapshotCommit(plan);
    expect(preparedClean.graphProjectionPlan.graphPaths).toEqual([caller]);
    setDirty(true);
    const observed: unknown[] = [];
    const stop = useResourceStore.subscribe((state) => {
      const key = resourceKey({ id: caller, kind: "event_graph" });
      observed.push({
        graphDirty: state.sessions[caller].saveDirty,
        dirty: state.documents[key]?.dirty,
        summaryDirty: state.resources[key]?.hasDirtyDocument,
        loaded: state.documents[key]?.loaded,
      });
    });
    try {
      await commitPreparedProjectSnapshot(preparedClean);
      expect(callerSnapshot()).toEqual(before);
      expect(observed).toEqual([
        { graphDirty: true, dirty: true, summaryDirty: true, loaded: true },
      ]);
    } finally {
      stop();
    }
  });
});

function editorPanel(panelInstanceId: string, resourceRef: string): WorkbenchPanelInfo {
  return {
    panelInstanceId,
    groupId: "editor-group",
    component: "EditorResource",
    title: resourceRef,
    metadata: { role: "editor", resourceRef, resourceKind: "function_graph" },
    active: false,
    location: { type: "grid" },
  };
}

function logsPanel(): WorkbenchPanelInfo {
  return {
    panelInstanceId: "logs-panel",
    groupId: "editor-group",
    component: "Logs",
    title: "Logs",
    metadata: { role: "view", viewId: "logs" },
    active: true,
    location: { type: "grid" },
  };
}

describe("editor FlexLayout publication commit", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    flexlayoutMocks.ready = true;
    flexlayoutMocks.panels.splice(
      0,
      flexlayoutMocks.panels.length,
      editorPanel("moved-panel", "functions/Old.yssbi-function"),
      editorPanel("stale-panel", "functions/Deleted.yssbi-function"),
      logsPanel(),
    );
  });

  it("commits shadow remap/removal and business stores before releasing stale pane state", async () => {
    const movedPath = "functions/New.yssbi-function";
    const movedResource = buildFileResourceMeta("function_graph", movedPath, "New");
    const commitBusinessStores = vi.fn();

    await commitEditorLayoutPublication(
      [{ from: "functions/Old.yssbi-function", to: movedPath }],
      { [movedResource.uri]: movedResource },
      new Set(),
      commitBusinessStores,
    );

    expect(flexlayoutMocks.runPublicationTransaction).toHaveBeenCalledOnce();
    expect(flexlayoutMocks.remapResource).toHaveBeenCalledWith(
      "functions/Old.yssbi-function",
      movedPath,
    );
    expect(flexlayoutMocks.removePanels).toHaveBeenCalledWith(["stale-panel"]);
    expect(commitBusinessStores).toHaveBeenCalledOnce();
    expect(flexlayoutMocks.panels.map((panel) => panel.panelInstanceId)).toEqual([
      "moved-panel",
      "logs-panel",
    ]);
    expect(flexlayoutMocks.panels[0].metadata).toMatchObject({
      role: "editor",
      resourceRef: movedPath,
    });
    expect(flexlayoutMocks.releasePane).toHaveBeenCalledOnce();
    expect(flexlayoutMocks.releasePane).toHaveBeenCalledWith("stale-panel");
  });

  it("preserves successor pane state after publication and during release notifications", async () => {
    for (const phase of ["receipt-owner", "receipt-panel", "release-owner", "release-panel"]) {
      const first = editorPanel("first", "functions/First.yssbi-function");
      const second = editorPanel("second", "functions/Second.yssbi-function");
      flexlayoutMocks.panels.splice(0, flexlayoutMocks.panels.length, first, second);
      flexlayoutMocks.releasePane.mockReset();
      let current = true;
      flexlayoutMocks.runPublicationTransaction.mockImplementationOnce(async (operation) => {
        const result = await operation({
          listPanels: () => flexlayoutMocks.panels,
          remapResource: flexlayoutMocks.remapResource,
          removePanels: flexlayoutMocks.removePanels,
        });
        if (phase === "receipt-owner") current = false;
        if (phase === "receipt-panel") flexlayoutMocks.panels.push(first);
        return result;
      });
      flexlayoutMocks.releasePane.mockImplementation((id: string) => {
        if (id !== first.panelInstanceId) return;
        if (phase === "release-owner") current = false;
        if (phase === "release-panel") flexlayoutMocks.panels.push(second);
      });
      await commitEditorLayoutPublication(
        [],
        {},
        new Set(),
        () => {},
        () => current,
      );
      const expected =
        phase === "receipt-owner"
          ? []
          : phase === "receipt-panel"
            ? [[second.panelInstanceId]]
            : [[first.panelInstanceId]];
      expect.soft(flexlayoutMocks.releasePane.mock.calls, phase).toEqual(expected);
    }
    flexlayoutMocks.releasePane.mockReset();
  });
});
