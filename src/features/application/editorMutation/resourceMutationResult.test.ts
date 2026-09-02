import { beforeEach, describe, expect, it, vi } from "vitest";
import type { WorkbenchPanelInfo } from "@/modules/workbench/internal/dockview/workbenchRead";
import { buildGraphResourceMeta } from "@/features/core/resource";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { useGraphDraftStore } from "@/features/core/graphDraft";
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

function callerSnapshot() {
  return structuredClone(useGraphProjectionStore.getState().graphEntities[caller]);
}

describe("resource mutation projection replacement protocol", () => {
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
    useGraphProjectionStore.getState().replaceProjection(caller, projection, 1);
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
  });

  it("does not replace a dirty Graph draft with a Rust projection publication", () => {
    const replacement = makeEditorProjectionFixture({ graphPath: caller, sourceRevision: 2 });
    useGraphDraftStore.setState({
      sessions: {
        [caller]: {
          document: { nodes: {}, port_bindings: [], connections: {}, input_states: [] },
          savedDocument: {
            nodes: {
              "00000000-0000-0000-0000-000000000001": {
                id: "00000000-0000-0000-0000-000000000001",
                node_type: "yssbi.tests.node",
                position: { x: 0, y: 0 },
                parameters: {},
                user_label: null,
              },
            },
            port_bindings: [],
            connections: {},
            input_states: [],
          },
          projection: replacement.projection,
          saving: false,
          undoStack: [],
          redoStack: [],
        },
      },
    });
    const result: ResourceMutationResultDto = {
      operationId: "00000000-0000-0000-0000-000000000905",
      projectInstanceId: "00000000-0000-0000-0000-000000000901",
      publicationRevision: 2,
      moves: [],
      deltas: [],
      projectionReplacements: [{ graphPath: caller, projection: replacement.projection }],
      projectionStatus: { status: "complete", expectedGraphPaths: [caller] },
      history: { canUndo: false, canRedo: false },
    };

    const plan = prepareSynchronousPublicationCommit(result, {
      projectInstanceId: result.projectInstanceId,
      epoch: 1,
      fingerprint: "dirty-draft-projection-suppression",
      affectedGraphPaths: new Set([caller]),
      moves: [],
    });

    expect(plan.projectionReplacements).toEqual([]);
    expect(plan.graphProjectionPlan?.graphEntities[caller].sourceRevision).toBe(1);
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
