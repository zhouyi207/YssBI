import { beforeEach, describe, expect, it, vi } from "vitest";
import { useGraphDataStore } from "@/features/core/dataStore/graphDataStore";
import { buildGraphResourceMeta, resourceKey, useResourceStore } from "@/features/core/resource";
import { projectPublicationCoordinator } from "./projectPublicationCoordinator";
import { makeEditorProjectionFixture } from "@/tests/helpers/editorProjectionFixtures";
import type { GraphMutationResultDto } from "@/shared/types/domain/editorMutation";
import { normalizeIpcError } from "@/services/ipc";
import { executeEditorMutation, resetEditorMutationCoordinator } from "./editorMutationCoordinator";

const projectInstanceId = "00000000-0000-0000-0000-000000000601";
const replacementId = "00000000-0000-0000-0000-000000000699";
const graphPath = "functions/Main.yssbi-function";
const operationId = "00000000-0000-0000-0000-000000000602";
const locale = "en-US";

function backendError(code: string) {
  return normalizeIpcError("mutate_graph_document", { code, details: null, incidentId: null });
}

function graphMutationResult(): GraphMutationResultDto {
  return {
    projectInstanceId,
    delta: {
      graphPath,
      fromRevision: 1,
      toRevision: 2,
      causedBy: operationId,
      payload: {
        operations: [
          {
            operation: "remove_node",
            node: {
              id: "00000000-0000-0000-0000-000000000604",
              node_type: "tests.node",
              position: { x: 0, y: 0 },
              parameters: {},
              user_label: null,
            },
          },
        ],
      },
    },
    projectionReplacement: {
      graphPath,
      projection: makeEditorProjectionFixture({
        graphPath,
        sourceRevision: 2,
        title: "Committed",
      }).projection,
    },
    history: { canUndo: true, canRedo: false },
  };
}

function deferred<T>(): {
  promise: Promise<T>;
  resolve(value: T): void;
} {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((settle) => {
    resolve = settle;
  });
  return { promise, resolve };
}

describe("executeEditorMutation lifecycle identity", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    resetEditorMutationCoordinator();
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
    useGraphDataStore.setState({ graphEntities: {} });
    useResourceStore.getState().clear();
    useResourceStore
      .getState()
      .upsertResource(buildGraphResourceMeta("function", graphPath, "Main", { revision: 1 }));
    useGraphDataStore
      .getState()
      .replaceProjection(
        graphPath,
        makeEditorProjectionFixture({ graphPath, sourceRevision: 1, title: "Current" }).projection,
        1,
      );
  });

  it("returns noop without installing projection, revision, history, or hydrating", async () => {
    const result = graphMutationResult();
    result.delta.toRevision = 1;
    result.delta.payload.operations = [];
    result.projectionReplacement.projection = makeEditorProjectionFixture({
      graphPath,
      sourceRevision: 1,
      title: "Ignored no-op replacement",
    }).projection;
    const functionProjection = result.projectionReplacement.functionEditorProjection;
    if (functionProjection) functionProjection.functionRevision = 1;
    const hydrateGraph = vi.fn();
    const updateHistoryStatus = vi.fn();

    await expect(
      executeEditorMutation(
        {
          graphPath,
          locale,
          mutation: { type: "deleteNodes", payload: { nodeIds: ["local-node"] } },
        },
        {
          createOperationId: () => operationId,
          mutateGraph: vi.fn().mockResolvedValue(result),
          hydrateGraph,
          updateHistoryStatus,
        },
      ),
    ).resolves.toEqual({ status: "noop", result });

    expect(useGraphDataStore.getState().graphEntities[graphPath]).toMatchObject({
      sourceRevision: 1,
      nodes: { "local-node": { title: "Current" } },
    });
    expect(
      useResourceStore.getState().resources[resourceKey({ id: graphPath, kind: "function" })]
        ?.revision,
    ).toBe(1);
    expect(updateHistoryStatus).not.toHaveBeenCalled();
    expect(hydrateGraph).not.toHaveBeenCalled();
  });

  it("rejects nonempty same-revision results and hydrates without installing them", async () => {
    const result = graphMutationResult();
    result.delta.toRevision = 1;
    result.delta.payload.operations = [
      {
        operation: "remove_node",
        node: {
          id: "00000000-0000-0000-0000-000000000604",
          node_type: "tests.node",
          position: { x: 0, y: 0 },
          parameters: {},
          user_label: null,
        },
      },
    ];
    result.projectionReplacement.projection = makeEditorProjectionFixture({
      graphPath,
      sourceRevision: 1,
      title: "Invalid replacement",
    }).projection;
    const hydrateGraph = vi.fn().mockResolvedValue(undefined);
    const updateHistoryStatus = vi.fn();

    await expect(
      executeEditorMutation(
        {
          graphPath,
          locale,
          mutation: { type: "deleteNodes", payload: { nodeIds: ["local-node"] } },
        },
        {
          createOperationId: () => operationId,
          mutateGraph: vi.fn().mockResolvedValue(result),
          hydrateGraph,
          updateHistoryStatus,
        },
      ),
    ).rejects.toThrow(/mutation result/i);

    expect(useGraphDataStore.getState().graphEntities[graphPath].sourceRevision).toBe(1);
    expect(updateHistoryStatus).not.toHaveBeenCalled();
    expect(hydrateGraph).toHaveBeenCalledWith(graphPath, locale);
  });

  it("rejects a mismatched result identity, hydrates once, and installs nothing", async () => {
    const result = graphMutationResult();
    result.projectInstanceId = replacementId;
    const hydrateGraph = vi.fn().mockResolvedValue(undefined);
    const updateHistoryStatus = vi.fn();

    await expect(
      executeEditorMutation(
        {
          graphPath,
          locale,
          mutation: { type: "deleteNodes", payload: { nodeIds: ["local-node"] } },
        },
        {
          createOperationId: () => operationId,
          mutateGraph: vi.fn().mockResolvedValue(result),
          hydrateGraph,
          updateHistoryStatus,
        },
      ),
    ).rejects.toThrow(/projectInstanceId/);

    expect(useGraphDataStore.getState().graphEntities[graphPath]).toMatchObject({
      sourceRevision: 1,
      nodes: { "local-node": { title: "Current" } },
    });
    expect(
      useResourceStore.getState().resources[resourceKey({ id: graphPath, kind: "function" })]
        ?.revision,
    ).toBe(1);
    expect(updateHistoryStatus).not.toHaveBeenCalled();
    expect(hydrateGraph).toHaveBeenCalledTimes(1);
    expect(hydrateGraph).toHaveBeenCalledWith(graphPath, locale);
  });

  it("advances sidebar resource revision after installing an authoritative graph mutation", async () => {
    await expect(
      executeEditorMutation(
        {
          graphPath,
          locale,
          mutation: { type: "deleteNodes", payload: { nodeIds: ["local-node"] } },
        },
        {
          createOperationId: () => operationId,
          mutateGraph: vi.fn().mockResolvedValue(graphMutationResult()),
          hydrateGraph: vi.fn(),
          updateHistoryStatus: vi.fn(),
        },
      ),
    ).resolves.toMatchObject({ status: "applied" });

    expect(
      useResourceStore.getState().resources[resourceKey({ id: graphPath, kind: "function" })]
        ?.revision,
    ).toBe(2);
  });

  it("passes one captured identity and ignores completion after project replacement", async () => {
    const result = deferred<GraphMutationResultDto>();
    const mutateGraph = vi.fn().mockReturnValue(result.promise);
    const applyStoreEffect = vi.fn();

    const completion = executeEditorMutation(
      {
        graphPath,
        locale,
        mutation: { type: "deleteNodes", payload: { nodeIds: ["local-node"] } },
      },
      {
        createOperationId: () => operationId,
        mutateGraph,
        hydrateGraph: vi.fn(),
        updateHistoryStatus: applyStoreEffect,
      },
    );

    expect(mutateGraph).toHaveBeenCalledWith(
      projectInstanceId,
      graphPath,
      locale,
      expect.any(Object),
    );
    projectPublicationCoordinator.startProject(replacementId, 0);
    const mutationResult = graphMutationResult();
    result.resolve(mutationResult);

    await expect(completion).resolves.toEqual({ status: "stale", result: mutationResult });
    expect(applyStoreEffect).not.toHaveBeenCalled();
    expect(useGraphDataStore.getState().graphEntities[graphPath].sourceRevision).toBe(1);
  });

  it("treats a backend stale lifecycle rejection as stale without store effects", async () => {
    const applyStoreEffect = vi.fn();
    const hydrateGraph = vi.fn();

    await expect(
      executeEditorMutation(
        {
          graphPath,
          locale,
          mutation: { type: "deleteNodes", payload: { nodeIds: ["local-node"] } },
        },
        {
          createOperationId: () => operationId,
          mutateGraph: vi.fn().mockRejectedValue(backendError("stale_project_lifecycle")),
          hydrateGraph,
          updateHistoryStatus: applyStoreEffect,
        },
      ),
    ).resolves.toEqual({ status: "stale" });

    expect(hydrateGraph).not.toHaveBeenCalled();
    expect(applyStoreEffect).not.toHaveBeenCalled();
    expect(useGraphDataStore.getState().graphEntities[graphPath].sourceRevision).toBe(1);
  });

  it("returns a typed rejection for a recognized graph validation code without hydrating or replaying", async () => {
    const mutateGraph = vi.fn().mockRejectedValue(backendError("graph_connection_type_mismatch"));
    const hydrateGraph = vi.fn();

    await expect(
      executeEditorMutation(
        {
          graphPath,
          locale,
          mutation: { type: "deleteNodes", payload: { nodeIds: ["local-node"] } },
        },
        {
          createOperationId: () => operationId,
          mutateGraph,
          hydrateGraph,
          updateHistoryStatus: vi.fn(),
        },
      ),
    ).resolves.toEqual({
      status: "rejected",
      code: "graph_connection_type_mismatch",
    });

    expect(mutateGraph).toHaveBeenCalledTimes(1);
    expect(hydrateGraph).not.toHaveBeenCalled();
  });

  it("hydrates exactly once for a revision conflict and never replays the mutation", async () => {
    const mutateGraph = vi.fn().mockRejectedValue(backendError("graph_revision_conflict"));
    const hydrateGraph = vi.fn().mockResolvedValue(undefined);

    await expect(
      executeEditorMutation(
        {
          graphPath,
          locale,
          mutation: { type: "deleteNodes", payload: { nodeIds: ["local-node"] } },
        },
        {
          createOperationId: () => operationId,
          mutateGraph,
          hydrateGraph,
          updateHistoryStatus: vi.fn(),
        },
      ),
    ).resolves.toEqual({ status: "conflict" });

    expect(mutateGraph).toHaveBeenCalledTimes(1);
    expect(hydrateGraph).toHaveBeenCalledTimes(1);
    expect(hydrateGraph).toHaveBeenCalledWith(graphPath, locale);
  });

  it("does not invoke the command when replacement occurs while reading revision authority", async () => {
    const capturedStore = useGraphDataStore.getState();
    vi.spyOn(useGraphDataStore, "getState").mockImplementationOnce(() => {
      projectPublicationCoordinator.startProject(replacementId, 0);
      return capturedStore;
    });
    const mutateGraph = vi.fn().mockResolvedValue(graphMutationResult());

    await expect(
      executeEditorMutation(
        {
          graphPath,
          locale,
          mutation: { type: "deleteNodes", payload: { nodeIds: ["local-node"] } },
        },
        {
          createOperationId: () => operationId,
          mutateGraph,
          hydrateGraph: vi.fn(),
          updateHistoryStatus: vi.fn(),
        },
      ),
    ).rejects.toMatchObject({ code: "stale_project_lifecycle" });

    expect(mutateGraph).not.toHaveBeenCalled();
    expect(useGraphDataStore.getState().graphEntities[graphPath].sourceRevision).toBe(1);
  });
});
