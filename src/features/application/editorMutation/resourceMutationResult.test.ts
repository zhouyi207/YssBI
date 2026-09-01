import { beforeEach, describe, expect, it, vi } from "vitest";
import type { WorkbenchPanelInfo } from "@/modules/workbench/internal/dockview/workbenchRead";
import { buildGraphResourceMeta } from "@/features/core/resource";
import { useGraphDataStore } from "@/features/core/dataStore/graphDataStore";
import { makeEditorProjectionFixture } from "@/tests/helpers/editorProjectionFixtures";
import type { ResourceMutationResultDto } from "@/shared/types/domain/editorMutation";
import { prepareSynchronousPublicationCommit } from "./resourceMutationResult";
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
const oldTarget = "functions/Old.yssbi-function";

function callerSnapshot() {
  return structuredClone(useGraphDataStore.getState().graphEntities[caller]);
}

describe("resource mutation projection replacement protocol", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    dockviewMocks.ready = true;
    dockviewMocks.panels.splice(0);
    useGraphDataStore.setState({ graphEntities: {} });
    const projection = makeEditorProjectionFixture({
      graphPath: caller,
      nodeId: "call-1",
      nodeTypeId: "yssbi.project.function.call",
      title: "Loaded caller",
    }).projection;
    useGraphDataStore.getState().replaceProjection(caller, projection, 1);
    useGraphDataStore.setState((state) => ({
      graphEntities: {
        ...state.graphEntities,
        [caller]: {
          ...state.graphEntities[caller],
          nodes: {
            ...state.graphEntities[caller].nodes,
            "call-1": {
              ...state.graphEntities[caller].nodes["call-1"],
              subGraphPath: oldTarget,
            },
          },
        },
      },
    }));
  });

  it("rejects complete status missing a caller replacement without locally patching the caller", () => {
    const before = callerSnapshot();
    const result: ResourceMutationResultDto = {
      operationId: "00000000-0000-0000-0000-000000000904",
      projectInstanceId: "00000000-0000-0000-0000-000000000901",
      publicationRevision: 1,
      moves: [],
      deltas: [],
      projectionReplacements: [],
      projectionStatus: { status: "complete", expectedGraphPaths: [caller] },
      history: { canUndo: false, canRedo: false },
    };

    expect(() =>
      prepareSynchronousPublicationCommit(result, {
        projectInstanceId: result.projectInstanceId,
        epoch: 1,
        fingerprint: "missing-loaded-caller-replacement",
        affectedGraphPaths: new Set([caller]),
        moves: [],
      }),
    ).toThrow("complete replacement paths do not equal the declared expected graph paths");
    expect(callerSnapshot()).toEqual(before);
    expect(useGraphDataStore.getState().graphEntities[caller]?.nodes["call-1"]?.subGraphPath).toBe(
      oldTarget,
    );
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
