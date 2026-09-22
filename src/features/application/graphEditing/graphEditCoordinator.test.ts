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
  resetGraphEditCoordinator,
} from "./graphEditCoordinator";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";

import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { GraphEditingService } from "@/services/nodeSystem/graphEditingService";
import { GraphProjectionService } from "@/services/nodeSystem/graphProjectionService";
import {
  hydrateGraphProjection,
  resetGraphProjectionLifecycle,
} from "@/features/application/graphProjection/graphProjectionLifecycle";

import type { GraphEditResultDto, GraphSaveResultDto } from "@/shared/types/domain/editorMutation";

const graphPath = "events/Queue.yssbi-event";
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
    useGraphProjectionStore.getState().clear();
    const projection = makeEditorProjectionFixture({ graphPath }).projection;
    useGraphProjectionStore.getState().install(graphPath, makeGraphEditorSession(projection));
  });

  it("orders edit and refresh without resolving the pre-edit document", async () => {
    const pendingEdit = deferred<GraphEditResultDto>();
    const pendingResolve = deferred<GraphEditResultDto>();
    const current = useGraphProjectionStore.getState().sessions[graphPath];
    const document = structuredClone(current.document);
    document.nodes["local-node"] = {
      id: "local-node",
      node_type: "tests.projected-node",
      position: { x: 10, y: 20 },
      parameters: {},
      user_label: "Edited",
    };
    const projection = structuredClone(current.projection);
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
    expect((await editing).status).toBe("applied");
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
    expect(useGraphProjectionStore.getState().sessions[graphPath].document).toEqual(document);
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

  it("releases the save lock before a queued refresh after Save fails", async () => {
    const pending = deferred<GraphSaveResultDto>();
    vi.spyOn(GraphEditingService, "save").mockReturnValueOnce(pending.promise);
    const session = useGraphProjectionStore.getState().sessions[graphPath];
    const hydrate = vi.spyOn(GraphProjectionService, "hydrateGraph").mockResolvedValueOnce({
      resultState: makeGraphEditorSession(session.projection).resultState,

      document: session.document,
      projection: session.projection,
      editing: makeGraphEditingState(),
    });
    const saving = saveGraph(graphPath, "event");
    const failedSave = expect(saving).rejects.toThrow("save failed");
    await vi.waitFor(() => expect(GraphEditingService.save).toHaveBeenCalledOnce());
    const refreshing = hydrateGraphProjection(graphPath, "en-US");
    expect(hydrate).not.toHaveBeenCalled();
    pending.reject(new Error("save failed"));
    await failedSave;
    expect(await refreshing).toBe(true);
    expect(useGraphProjectionStore.getState().sessions[graphPath].saving).toBe(false);
  });

  it("rejects an invalid mutation projection before changing draft or history", async () => {
    const before = structuredClone(useGraphProjectionStore.getState().sessions[graphPath]);
    const projection = structuredClone(before.projection);
    projection.nodes.push(structuredClone(projection.nodes[0]));
    await expect(
      applyGraphMutation(
        { graphPath, locale: "en-US", mutation: { type: "moveNodes", payload: { positions: [] } } },
        {
          transform: async () => ({
            resultState: makeGraphEditorSession(projection).resultState,

            changed: true,
            document: before.document,
            projection,
            editing: makeGraphEditingState(),
          }),
        },
      ),
    ).rejects.toThrow("duplicate node");
    expect(useGraphProjectionStore.getState().sessions[graphPath]).toEqual(before);
  });
});
