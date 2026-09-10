import { beforeEach, describe, expect, it, vi } from "vitest";
import type { WorkbenchPanelInfo } from "@/modules/workbench/internal/dockview/workbenchRead";
import { buildGraphResourceMeta } from "@/features/core/resource";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { useGraphDraftStore } from "@/features/core/graphDraft";
import {
  makeEditorProjectionFixture,
  makeGraphEditorSession,
} from "@/tests/helpers/editorProjectionFixtures";
import {
  captureProjectIdentity,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import {
  prepareProjectSnapshotCommit,
  commitPreparedProjectSnapshot,
} from "./projectPublicationSnapshot";
import { commitEditorDockviewPublication } from "./editorDockviewPublicationCommit";

const dockviewMocks = vi.hoisted(() => {
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

vi.mock("@/modules/workbench/internal/dockview/workbenchRead", () => ({
  workbenchDockviewRead: {
    get isReady() {
      return dockviewMocks.ready;
    },
  },
}));

vi.mock("@/modules/workbench/internal/dockview/workbenchDockviewInternal", () => ({
  workbenchDockviewRuntime: { control: {} },
  workbenchDockviewInternal: {
    runPublicationTransaction: dockviewMocks.runPublicationTransaction,
  },
}));

vi.mock("@/modules/workbench/internal/dockview/editorPaneStateStore", () => ({
  useEditorPaneStateStore: {
    getState: () => ({ release: dockviewMocks.releasePane }),
  },
}));

const caller = "events/Caller.yssbi-event";

function callerSnapshot() {
  return structuredClone(useGraphProjectionStore.getState().graphEntities[caller]);
}

describe("project snapshot projection replacement", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    dockviewMocks.ready = true;
    dockviewMocks.panels.splice(0);
    useGraphProjectionStore.setState({ graphEntities: {} });
    useGraphDraftStore.getState().clear();
    const projection = makeEditorProjectionFixture({
      graphPath: caller,
      nodeId: "call-1",
      nodeTypeId: "yssbi.project.function.call",
      title: "Loaded caller",
    }).projection;
    useGraphProjectionStore.getState().replaceProjection(caller, projection);
  });

  it("does not replace a dirty Graph draft, including edits made after snapshot preparation", async () => {
    const replacement = makeEditorProjectionFixture({ graphPath: caller });
    useGraphDraftStore.getState().install(caller, makeGraphEditorSession(replacement.projection));
    const setDirty = (saveDirty: boolean) =>
      useGraphDraftStore.setState((state) => ({
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
        graphs: [{ path: caller, name: "Caller", type: "event", revision: 2 }],
        charts: [],
        databases: [],
      },
      graphSessions: new Map([
        [
          caller,
          {
            document: useGraphDraftStore.getState().sessions[caller].savedDocument,
            projection: replacement.projection,
          },
        ],
      ]),
      chartDocuments: new Map(),
      pathRemaps: new Map(),
      chartPathRemaps: new Map(),
    });

    expect(plan.graphProjectionPlan.graphPaths).toEqual([]);
    const before = callerSnapshot();
    expect(plan.graphProjectionPlan.graphEntities[caller]).toEqual(before);
    setDirty(false);
    const preparedClean = prepareProjectSnapshotCommit(plan);
    expect(preparedClean.graphProjectionPlan.graphPaths).toEqual([caller]);
    setDirty(true);
    await commitPreparedProjectSnapshot(preparedClean);
    expect(callerSnapshot()).toEqual(before);
  });
});

function editorPanel(panelInstanceId: string, resourceRef: string): WorkbenchPanelInfo {
  return {
    panelInstanceId,
    groupId: "editor-group",
    component: "EditorResource",
    title: resourceRef,
    metadata: { role: "editor", resourceRef, resourceKind: "function" },
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

describe("editor Dockview publication commit", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    dockviewMocks.ready = true;
    dockviewMocks.panels.splice(
      0,
      dockviewMocks.panels.length,
      editorPanel("moved-panel", "functions/Old.yssbi-function"),
      editorPanel("stale-panel", "functions/Deleted.yssbi-function"),
      logsPanel(),
    );
  });

  it("commits shadow remap/removal and business stores before releasing stale pane state", async () => {
    const movedPath = "functions/New.yssbi-function";
    const movedResource = buildGraphResourceMeta("function", movedPath, "New");
    const commitBusinessStores = vi.fn();

    await commitEditorDockviewPublication(
      [{ from: "functions/Old.yssbi-function", to: movedPath }],
      { [movedResource.uri]: movedResource },
      commitBusinessStores,
    );

    expect(dockviewMocks.runPublicationTransaction).toHaveBeenCalledOnce();
    expect(dockviewMocks.remapResource).toHaveBeenCalledWith(
      "functions/Old.yssbi-function",
      movedPath,
    );
    expect(dockviewMocks.removePanels).toHaveBeenCalledWith(["stale-panel"]);
    expect(commitBusinessStores).toHaveBeenCalledOnce();
    expect(dockviewMocks.panels.map((panel) => panel.panelInstanceId)).toEqual([
      "moved-panel",
      "logs-panel",
    ]);
    expect(dockviewMocks.panels[0].metadata).toMatchObject({
      role: "editor",
      resourceRef: movedPath,
    });
    expect(dockviewMocks.releasePane).toHaveBeenCalledOnce();
    expect(dockviewMocks.releasePane).toHaveBeenCalledWith("stale-panel");
  });
});
