import {
  makeEditorProjectionFixture,
  makeGraphEditorSession,
  makeGraphEditingState,
} from "@/tests/helpers/editorProjectionFixtures";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { saveGraph } from "./saveGraph";
import {
  applyGraphMutation,
  enqueueGraphTask,
  installGraphSession,
  resetGraphEditCoordinator,
} from "./graphEditCoordinator";

import {
  buildFileResourceMeta,
  markResourceStale,
  resourceKey,
  useResourceStore,
} from "@/features/core/resource";

import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { GraphEditingService } from "@/services/nodeSystem/graphEditingService";
import { GraphProjectionService } from "@/services/nodeSystem/graphProjectionService";
import { useExecutionStore } from "@/features/core/execution/useExecutionStore";
import { normalizeIpcError } from "@/services/ipc/ipcError";
import { logger } from "@/utils/frontendLogger";
import {
  hydrateGraphProjection,
  loadGraphProjection,
  resetGraphProjectionLifecycle,
} from "@/features/application/graphProjection/graphProjectionLifecycle";
import * as graphActivity from "@/features/application/graphProjection/graphActivity";

import type { GraphEditResultDto, GraphSaveResultDto } from "@/shared/types/domain/editorMutation";

const graphPath = "events/Queue.yssbi-event";
let initialSession: ReturnType<typeof makeGraphEditorSession>;
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

describe("Graph draft task ordering", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    clearProjectLifecycle();
    startProjectLifecycle("queue-project");
    resetGraphEditCoordinator();
    resetGraphProjectionLifecycle();
    useResourceStore.getState().clear();
    const projection = makeEditorProjectionFixture({ graphPath }).projection;
    initialSession = makeGraphEditorSession(projection);
    useResourceStore.getState().installGraphSession(graphPath, initialSession, { mode: "load" });
  });

  it("keeps an edited graph ready while activity refreshes schema at the same revision", async () => {
    useResourceStore.getState().setSnapshot({
      resources: [buildFileResourceMeta("event_graph", graphPath, "Queue", { revision: 0 })],
    });
    const activity = vi.spyOn(graphActivity, "ensureGraphActivity").mockResolvedValue();
    vi.spyOn(GraphProjectionService, "loadGraph").mockResolvedValue(initialSession);
    expect(await loadGraphProjection(graphPath)).toBe(true);
    const refresh = activity.mock.calls[0]![1];
    const pendingEdit = deferred<GraphEditResultDto>();
    const pendingRefresh = deferred<ReturnType<typeof makeGraphEditorSession>>();
    const transform = vi
      .spyOn(GraphEditingService, "transform")
      .mockReturnValue(pendingEdit.promise);
    const hydrate = vi
      .spyOn(GraphProjectionService, "hydrateGraph")
      .mockReturnValue(pendingRefresh.promise);
    const edited = structuredClone(initialSession);
    edited.editing = makeGraphEditingState({
      dirty: true,
      canUndo: true,
      version: { ...edited.editing.version, revision: "1" },
    });
    edited.projection.nodes[0].position = { x: 100, y: 200 };
    const observed = structuredClone(edited);
    observed.projection.nodes[0].ports[0].resolvedSchema = {
      kind: "derived",
      fields: [{ name: "result", scalarType: "Numeric" }],
    };
    const key = resourceKey({ id: graphPath, kind: "event_graph" });
    const readiness: boolean[] = [];
    const stop = useResourceStore.subscribe((state) => {
      const document = state.documents[key];
      readiness.push(document.loaded && !document.stale && !document.conflict);
    });
    try {
      const editing = applyGraphMutation({
        graphPath,
        mutation: {
          type: "moveNodes",
          payload: {
            positions: [
              { nodeId: edited.projection.nodes[0].nodeId, position: { x: 100, y: 200 } },
            ],
          },
        },
      });
      await vi.waitFor(() => expect(transform).toHaveBeenCalledOnce());
      const refreshing = refresh(graphPath, edited.editing);
      expect(hydrate).not.toHaveBeenCalled();
      pendingEdit.resolve({ ...edited, changed: true });
      expect(await editing).toMatchObject({ status: "applied" });
      await vi.waitFor(() => expect(hydrate).toHaveBeenCalledOnce());
      expect.soft(useResourceStore.getState().documents[key].stale).toBe(false);
      expect(useResourceStore.getState().sessions[graphPath].version.revision).toBe("1");
      pendingRefresh.resolve(observed);
      expect(await refreshing).toBe(true);
      expect(useResourceStore.getState().sessions[graphPath].projection).toEqual(
        observed.projection,
      );
      expect(readiness.length).toBeGreaterThan(0);
      expect(readiness.every(Boolean)).toBe(true);
    } finally {
      stop();
      pendingEdit.resolve({ ...edited, changed: true });
      pendingRefresh.resolve(observed);
    }
  });

  it("orders edit and refresh without resolving the pre-edit document", async () => {
    const pendingEdit = deferred<GraphEditResultDto>();
    const pendingResolve = deferred<GraphEditResultDto>();
    const current = useResourceStore.getState().sessions[graphPath];
    const document = structuredClone(initialSession.document);
    const insertedId = "00000000-0000-0000-0000-000000000005";
    document.nodes[insertedId] = {
      id: insertedId,
      node_type: "tests.projected-node",
      position: { x: 10, y: 20 },
      parameters: {},
      user_label: "Edited",
    };
    const projection = structuredClone(current.projection);
    projection.nodes.push(
      makeEditorProjectionFixture({ graphPath, nodeId: insertedId }).projection.nodes[0],
    );
    projection.basis.semanticInputHash = "1".repeat(64);
    const transform = vi
      .spyOn(GraphEditingService, "transform")
      .mockReturnValueOnce(pendingEdit.promise);
    const resolve = vi
      .spyOn(GraphEditingService, "resolve")
      .mockReturnValue(pendingResolve.promise);
    const hydrate = vi.spyOn(GraphProjectionService, "hydrateGraph");
    const editing = applyGraphMutation({
      graphPath,
      mutation: { type: "moveNodes", payload: { positions: [] } },
    });
    await vi.waitFor(() => expect(transform).toHaveBeenCalledOnce());
    const refreshing = hydrateGraphProjection(graphPath, "en-US");
    expect(resolve).not.toHaveBeenCalled();
    pendingEdit.resolve({
      resultState: makeGraphEditorSession(projection).resultState,

      changed: true,
      document,
      projection,
      editing: makeGraphEditingState({
        dirty: true,
        canUndo: true,
        version: { ...current.version, revision: "1" },
      }),
    });
    expect(await editing).toMatchObject({ status: "applied", insertedNodeIds: [insertedId] });
    await vi.waitFor(() => expect(resolve).toHaveBeenCalledOnce());
    expect(resolve.mock.calls[0]?.[3]).toEqual({ ...current.version, revision: "1" });
    expect(hydrate).not.toHaveBeenCalled();
    pendingResolve.resolve({
      resultState: makeGraphEditorSession(projection).resultState,

      changed: false,
      document,
      projection,
      editing: makeGraphEditingState({
        dirty: true,
        canUndo: true,
        version: { ...current.version, revision: "1" },
      }),
    });
    expect(await refreshing).toBe(true);
    expect(useResourceStore.getState().sessions[graphPath].projection).toEqual(projection);
    expect(useResourceStore.getState().graphEntities[graphPath].nodes[insertedId]).toBeDefined();
  });

  it("publishes each editing and save receipt's resource flags and revision together", async () => {
    const ref = { id: graphPath, kind: "event_graph" as const };
    const key = resourceKey(ref);
    useResourceStore.getState().setSnapshot({
      resources: [buildFileResourceMeta(ref.kind, graphPath, "Queue", { revision: 0 })],
    });
    markResourceStale(ref);
    const before = useResourceStore.getState();
    const observed: unknown[] = [];
    const stop = useResourceStore.subscribe((state, previous) => {
      if (state.resources === previous.resources && state.documents === previous.documents) return;
      observed.push({
        revision: state.resources[key].revision,
        dirty: state.documents[key].dirty,
        stale: state.documents[key].stale,
        summaryDirty: state.resources[key].hasDirtyDocument,
        summaryStale: state.resources[key].hasStaleDocument,
      });
    });
    try {
      const session = makeGraphEditorSession(makeEditorProjectionFixture({ graphPath }).projection);
      session.editing = makeGraphEditingState({
        dirty: true,
        version: { ...session.editing.version, revision: "3" },
      });
      expect(installGraphSession(graphPath, session)).toBe(true);
      expect
        .soft(observed)
        .toEqual([
          { revision: 3, dirty: true, stale: false, summaryDirty: true, summaryStale: false },
        ]);
      observed.length = 0;
      vi.spyOn(GraphEditingService, "save").mockResolvedValue({
        projectInstanceId: "queue-project",
        resourceRevision: 9,
        document: session.document,
        editing: { ...session.editing, dirty: false },
        projectionReplacement: { graphPath, projection: session.projection },
        resultState: session.resultState,
      });
      expect(await saveGraph(graphPath, ref.kind)).toBe(true);
      expect(observed).toEqual([
        { revision: 9, dirty: false, stale: false, summaryDirty: false, summaryStale: false },
      ]);
      expect(before.resources[key].revision).toBe(0);
      expect(before.documents[key].stale).toBe(true);
    } finally {
      stop();
    }
  });

  it("publishes a graph frame and its resource state before graph subscribers run", () => {
    const ref = { id: graphPath, kind: "event_graph" as const };
    const key = resourceKey(ref);
    useResourceStore.getState().setSnapshot({
      resources: [buildFileResourceMeta(ref.kind, graphPath, "Queue", { revision: 0 })],
    });
    const session = makeGraphEditorSession(makeEditorProjectionFixture({ graphPath }).projection);
    session.editing = makeGraphEditingState({
      dirty: true,
      version: { ...session.editing.version, revision: "3" },
    });
    const observed: unknown[] = [];
    const stop = useResourceStore.subscribe((state) => {
      const resource = useResourceStore.getState();
      observed.push({
        graphRevision: state.sessions[graphPath].version.revision,
        graphDirty: state.sessions[graphPath].saveDirty,
        resourceRevision: resource.resources[key].revision,
        loaded: resource.documents[key]?.loaded,
        dirty: resource.documents[key]?.dirty,
      });
    });
    try {
      expect(installGraphSession(graphPath, session)).toBe(true);
      expect(observed).toEqual([
        { graphRevision: "3", graphDirty: true, resourceRevision: 3, loaded: true, dirty: true },
      ]);
      expect(installGraphSession(graphPath, session)).toBe(true);
      expect(observed).toHaveLength(1);
    } finally {
      stop();
    }
  });

  it("rejects queue overflow while allowing another graph to proceed and releases capacity", async () => {
    const blocked = deferred<number>();
    const queued = Array.from({ length: 64 }, () =>
      enqueueGraphTask(graphPath, () => blocked.promise, -1),
    );
    const overflow = vi.fn(async () => 0);
    await expect(enqueueGraphTask(graphPath, overflow, -1)).rejects.toMatchObject({
      code: "graph_edit_busy",
    });
    await expect(enqueueGraphTask("events/Other.yssbi-event", async () => 7, -1)).resolves.toBe(7);
    blocked.resolve(1);
    await Promise.all(queued);
    await expect(enqueueGraphTask(graphPath, async () => 2, -1)).resolves.toBe(2);
    expect(overflow).not.toHaveBeenCalled();
  });

  it("stops graph publication reconciliation when a listener replaces its installed session", () => {
    for (const replaceProject of [false, true]) {
      useResourceStore.getState().clear();
      useResourceStore.getState().installGraphSession(graphPath, initialSession, { mode: "load" });
      const replacement = structuredClone(initialSession);
      replacement.editing.version.revision = "1";
      replacement.editing.dirty = true;
      let replaced = false;
      let ownsSuccessorRun: (() => boolean) | undefined;
      const stop = useResourceStore.subscribe(() => {
        if (replaced) return;
        replaced = true;
        if (replaceProject) startProjectLifecycle("successor-project");
        useResourceStore.getState().removeGraphSession(graphPath);
        useResourceStore
          .getState()
          .installGraphSession(graphPath, initialSession, { mode: "load" });
        ownsSuccessorRun = useExecutionStore.getState().submitExecution(graphPath);
      });
      try {
        expect.soft(installGraphSession(graphPath, replacement)).toBe(false);
        expect.soft(replaced).toBe(true);
        expect.soft(ownsSuccessorRun?.()).toBe(true);
        expect.soft(useResourceStore.getState().sessions[graphPath].version.revision).toBe("0");
      } finally {
        stop();
        useExecutionStore.setState({ graphs: {} });
      }
    }
  });

  it("stops projection refresh when stale-state notification replaces the project", async () => {
    const resolve = vi.spyOn(GraphEditingService, "resolve").mockResolvedValue({
      ...initialSession,
      changed: false,
    });
    const hydrate = vi
      .spyOn(GraphProjectionService, "hydrateGraph")
      .mockResolvedValue(initialSession);
    const ref = { id: graphPath, kind: "event_graph" as const };
    const key = resourceKey(ref);
    for (const dirty of [false, true]) {
      resolve.mockClear();
      hydrate.mockClear();
      startProjectLifecycle("queue-project");
      useResourceStore.getState().clear();
      useResourceStore.getState().setSnapshot({
        resources: [buildFileResourceMeta(ref.kind, graphPath, "Queue", { revision: 0 })],
      });
      const session = { ...initialSession, editing: { ...initialSession.editing, dirty } };
      useResourceStore.getState().installGraphSession(graphPath, session, { mode: "load" });
      let successor: ReturnType<typeof useResourceStore.getState> | undefined;
      let replaced = false;
      const stop = useResourceStore.subscribe((state) => {
        if (replaced || !state.documents[key]?.stale) return;
        replaced = true;
        startProjectLifecycle("successor-project");
        useResourceStore.getState().clear();
        useResourceStore.getState().setSnapshot({
          resources: [buildFileResourceMeta(ref.kind, graphPath, "Successor", { revision: 0 })],
        });
        useResourceStore.getState().installGraphSession(graphPath, session, { mode: "load" });
        successor = useResourceStore.getState();
      });
      try {
        expect.soft(await hydrateGraphProjection(graphPath, "en-US")).toBe(false);
        expect.soft(replaced).toBe(true);
        expect.soft(resolve).not.toHaveBeenCalled();
        expect.soft(hydrate).not.toHaveBeenCalled();
        expect.soft(useResourceStore.getState()).toBe(successor);
      } finally {
        stop();
      }
    }
  });

  it("releases the save lock before a queued refresh after Save fails", async () => {
    const pending = deferred<GraphSaveResultDto>();
    vi.spyOn(GraphEditingService, "save").mockReturnValueOnce(pending.promise);
    const session = useResourceStore.getState().sessions[graphPath];
    const hydrate = vi.spyOn(GraphProjectionService, "hydrateGraph").mockResolvedValueOnce({
      resultState: makeGraphEditorSession(session.projection).resultState,

      document: initialSession.document,
      projection: session.projection,
      editing: makeGraphEditingState(),
    });
    const saving = saveGraph(graphPath, "event_graph");
    const failedSave = expect(saving).rejects.toThrow("save failed");
    await vi.waitFor(() => expect(GraphEditingService.save).toHaveBeenCalledOnce());
    const refreshing = hydrateGraphProjection(graphPath, "en-US");
    expect(hydrate).not.toHaveBeenCalled();
    pending.reject(new Error("save failed"));
    await failedSave;
    expect(await refreshing).toBe(true);
    expect(useResourceStore.getState().sessions[graphPath].saving).toBe(false);
  });

  it("does not save or unlock a successor installed during save notifications", async () => {
    const save = vi
      .spyOn(GraphEditingService, "save")
      .mockRejectedValue(new Error("obsolete save"));
    for (const reopen of [false, true]) {
      save.mockClear();
      useResourceStore.getState().clear();
      useResourceStore.getState().installGraphSession(graphPath, initialSession, { mode: "load" });
      let replaced = false;
      let successor: ReturnType<typeof useResourceStore.getState>["sessions"][string] | undefined;
      const stop = useResourceStore.subscribe((state) => {
        if (replaced || !state.sessions[graphPath]?.saving) return;
        replaced = true;
        useResourceStore.getState().removeGraphSession(graphPath);
        if (reopen) {
          useResourceStore
            .getState()
            .installGraphSession(graphPath, initialSession, { mode: "load" });
          useResourceStore.getState().beginGraphSave(graphPath);
          successor = useResourceStore.getState().sessions[graphPath];
        }
      });
      try {
        const [result] = await Promise.allSettled([saveGraph(graphPath, "event_graph")]);
        expect.soft(result).toEqual({ status: "fulfilled", value: false });
        expect.soft(save).not.toHaveBeenCalled();
        expect.soft(useResourceStore.getState().sessions[graphPath]).toBe(successor);
      } finally {
        stop();
      }
    }

    // A successful save releases its lock before subscribers can admit another save
    // on the same loaded graph. Its finally must not unlock that second request.
    useResourceStore.getState().clear();
    useResourceStore.getState().installGraphSession(graphPath, initialSession, { mode: "load" });
    const receipt: GraphSaveResultDto = {
      projectInstanceId: "queue-project",
      resourceRevision: 1,
      document: initialSession.document,
      editing: initialSession.editing,
      projectionReplacement: { graphPath, projection: initialSession.projection },
      resultState: initialSession.resultState,
    };
    const pending = deferred<GraphSaveResultDto>();
    save.mockClear();
    save.mockResolvedValueOnce(receipt).mockReturnValueOnce(pending.promise);
    let successorSave: Promise<boolean> | undefined;
    let admitted = false;
    const stop = useResourceStore.subscribe((state, previous) => {
      if (
        admitted ||
        !previous.sessions[graphPath]?.saving ||
        state.sessions[graphPath]?.saving !== false
      )
        return;
      admitted = true;
      successorSave = saveGraph(graphPath, "event_graph");
    });
    try {
      expect.soft(await saveGraph(graphPath, "event_graph")).toBe(true);
      await vi.waitFor(() => expect(save).toHaveBeenCalledTimes(2));
      expect.soft(useResourceStore.getState().sessions[graphPath].saving).toBe(true);
    } finally {
      stop();
      pending.resolve(receipt);
      await successorSave;
    }
  });

  it("logs only failure identity when projection refresh rejects private error content", async () => {
    const report = vi.spyOn(logger.graph, "error").mockImplementation(() => {});
    const rawMessage = vi.fn(() => "private document content");
    const raw = new Error();
    Object.defineProperty(raw, "message", { get: rawMessage });
    const failures = [
      { error: raw, identity: "ipc_transport_failure" },
      {
        error: normalizeIpcError("hydrate_graph_projection", {
          code: "graph_not_found",
          details: { privatePayload: "private document content" },
          incidentId: "incident-refresh",
        }),
        identity: "graph_not_found (incident-refresh)",
      },
      {
        error: { code: "private document content", message: "private document content" },
        identity: "ipc_malformed_error",
      },
    ];
    const hydrate = vi.spyOn(GraphProjectionService, "hydrateGraph");
    for (const { error, identity } of failures) {
      report.mockClear();
      hydrate.mockRejectedValueOnce(error);
      await expect(hydrateGraphProjection(graphPath, "en-US")).resolves.toBe(false);
      expect(report).toHaveBeenCalledExactlyOnceWith(
        `Graph projection hydrate IPC failed for '${graphPath}': ${identity}`,
        "GraphProjectionLifecycle",
      );
    }
    expect(rawMessage).not.toHaveBeenCalled();
  });

  it("rejects an invalid mutation projection before changing draft or history", async () => {
    const before = structuredClone(useResourceStore.getState().sessions[graphPath]);
    const projection = structuredClone(before.projection);
    projection.nodes.push(structuredClone(projection.nodes[0]));
    await expect(
      applyGraphMutation(
        { graphPath, locale: "en-US", mutation: { type: "moveNodes", payload: { positions: [] } } },
        {
          transform: async () => ({
            resultState: makeGraphEditorSession(projection).resultState,

            changed: true,
            document: initialSession.document,
            projection,
            editing: makeGraphEditingState(),
          }),
        },
      ),
    ).rejects.toThrow("duplicate node");
    expect(useResourceStore.getState().sessions[graphPath]).toEqual(before);
  });
});
