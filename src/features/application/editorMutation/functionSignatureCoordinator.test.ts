import { projectIndexSnapshotFixture } from "@/tests/helpers/activityPanelFixture";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { useGraphMetaStore } from "@/features/core/dataStore/graphMetaStore";
import { buildGraphResourceMeta, useResourceStore } from "@/features/core/resource";
import type {
  FunctionSignatureDto,
  ResourceMutationResultDto,
} from "@/shared/types/domain/editorMutation";
import {
  makeEditorProjectionFixture,
  makeGraphEditorSession,
} from "@/tests/helpers/editorProjectionFixtures";
import { GraphProjectionService } from "@/services/nodeSystem/graphProjectionService";
import { ProjectService } from "@/services/project/projectService";
import {
  executeFunctionSignatureMutation,
  resetFunctionSignatureCoordinator,
  type FunctionSignatureCoordinatorDependencies,
} from "./functionSignatureCoordinator";
import { projectPublicationCoordinator } from "./projectPublicationCoordinator";

const functionPath = "functions/Compute.yssbi-function";
const operationId = "00000000-0000-0000-0000-000000000501";
const projectInstanceId = "00000000-0000-0000-0000-000000000601";

const beforeSignature: FunctionSignatureDto = {
  parameters: [{ id: "value", name: "Value", type_name: "Int64" }],
  return_type: "Float64",
};
const afterSignature: FunctionSignatureDto = {
  parameters: [{ id: "value", name: "Renamed", type_name: "Float64" }],
  return_type: "Int64",
};
const authoritativeFunctionProjection = {
  functionRevision: 3,
  inputs: [{ id: "value", name: "Observed value", dataType: { kind: "Float64" as const } }],
  outputs: [
    {
      id: "computed",
      name: "Computed value",
      dataType: { kind: "Struct" as const, inner: "RegressionModel" },
    },
  ],
};

vi.mock("@/services/nodeSystem/graphProjectionService", () => ({
  GraphProjectionService: {
    loadGraph: vi.fn(),
    hydrateGraph: vi.fn(),
  },
}));

function installState(): void {
  useGraphProjectionStore.setState({ graphEntities: {} });
  useResourceStore.getState().clear();
  useResourceStore
    .getState()
    .upsertResource(buildGraphResourceMeta("function", functionPath, "Compute", { revision: 2 }));
  useGraphProjectionStore.getState().replaceProjection(
    functionPath,
    makeEditorProjectionFixture({
      graphPath: functionPath,
      title: "Current graph projection",
    }).projection,
  );
  useGraphMetaStore.setState({
    graphs: {
      [functionPath]: {
        path: functionPath,
        name: "Compute",
        type: "function",
        functionRevision: 2,
        functionSignature: beforeSignature,
        functionInputs: [{ id: "value", name: "Value", dataType: { kind: "Int64" } }],
        functionOutputs: [{ id: "return", name: "Result", dataType: { kind: "Float64" } }],
      },
    },
  });
}

function result(
  projectionStatus: ResourceMutationResultDto["projectionStatus"],
  withReplacement: boolean,
): ResourceMutationResultDto {
  return {
    operationId,
    projectInstanceId,
    publicationRevision: 1,
    moves: [],
    deltas: [
      {
        resource: { kind: "function", key: functionPath },
        fromRevision: 2,
        toRevision: 3,
        causedBy: operationId,
        payload: {
          kind: "function",
          patch: { before: beforeSignature, after: afterSignature },
        },
      },
    ],
    projectionReplacements: withReplacement
      ? [
          {
            graphPath: functionPath,
            projection: makeEditorProjectionFixture({
              graphPath: functionPath,
              title: "Committed signature projection",
            }).projection,
            functionEditorProjection: authoritativeFunctionProjection,
          },
        ]
      : [],
    projectionStatus,
  };
}

function dependencies(
  mutateSignature: FunctionSignatureCoordinatorDependencies["mutateSignature"],
  hydrateGraph = vi.fn(async () => true),
  refreshResourceIndex: FunctionSignatureCoordinatorDependencies["refreshResourceIndex"] = () =>
    projectPublicationCoordinator.refreshIndex(),
): Partial<FunctionSignatureCoordinatorDependencies> {
  return {
    mutateSignature,
    hydrateGraph,
    refreshResourceIndex,
  };
}

describe("executeFunctionSignatureMutation", () => {
  afterEach(() => vi.restoreAllMocks());

  beforeEach(() => {
    vi.restoreAllMocks();
    vi.clearAllMocks();
    vi.spyOn(crypto, "randomUUID").mockReturnValue(operationId);
    vi.mocked(GraphProjectionService.loadGraph).mockImplementation(async (graphPath) =>
      makeGraphEditorSession(makeEditorProjectionFixture({ graphPath }).projection),
    );
    resetFunctionSignatureCoordinator();
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
    installState();
  });

  it("discards a successful response from before a coordinator reset", async () => {
    let resolveOld!: (value: ResourceMutationResultDto) => void;
    const old = new Promise<ResourceMutationResultDto>((resolve) => {
      resolveOld = resolve;
    });
    const mutateSignature = vi.fn().mockReturnValueOnce(old);
    const overrides = dependencies(mutateSignature);
    const input = { functionPath, locale: "en-US", patch: { inputs: [] } };
    const submit = vi.spyOn(projectPublicationCoordinator, "submit");
    const beforeMeta = useGraphMetaStore.getState().graphs[functionPath];
    const beforeGraph = useGraphProjectionStore.getState().graphEntities[functionPath];
    const oldRequest = executeFunctionSignatureMutation(input, overrides);
    resetFunctionSignatureCoordinator();
    const committed = result({ status: "complete", expectedGraphPaths: [functionPath] }, true);
    resolveOld(committed);

    await expect(oldRequest).resolves.toEqual({ status: "stale", result: committed });
    expect(submit).not.toHaveBeenCalled();
    expect(useGraphMetaStore.getState().graphs[functionPath]).toBe(beforeMeta);
    expect(useGraphProjectionStore.getState().graphEntities[functionPath]).toBe(beforeGraph);
  });

  it("does not invoke, publish, or mutate when project replacement occurs inside authority read", async () => {
    const authority = useGraphMetaStore.getState();
    vi.spyOn(useGraphMetaStore, "getState").mockImplementationOnce(() => {
      projectPublicationCoordinator.startProject("00000000-0000-0000-0000-000000000602", 0);
      return authority;
    });
    const mutateSignature = vi.fn(async () =>
      result({ status: "complete", expectedGraphPaths: [functionPath] }, true),
    );
    const submit = vi.spyOn(projectPublicationCoordinator, "submit");
    const beforeMeta = authority.graphs[functionPath];
    const beforeGraph = useGraphProjectionStore.getState().graphEntities[functionPath];

    await expect(
      executeFunctionSignatureMutation(
        {
          functionPath,
          locale: "en-US",
          patch: { inputs: [] },
        },
        dependencies(mutateSignature),
      ),
    ).rejects.toMatchObject({ code: "stale_project_lifecycle" });

    expect(mutateSignature).not.toHaveBeenCalled();
    expect(submit).not.toHaveBeenCalled();
    expect(useGraphMetaStore.getState().graphs[functionPath]).toBe(beforeMeta);
    expect(useGraphProjectionStore.getState().graphEntities[functionPath]).toBe(beforeGraph);
  });

  it("rejects missing signature authority before invoke or publication effects", async () => {
    useGraphMetaStore.getState().clear();
    const mutateSignature = vi.fn();
    const submit = vi.spyOn(projectPublicationCoordinator, "submit");

    await expect(
      executeFunctionSignatureMutation(
        {
          functionPath,
          locale: "en-US",
          patch: { inputs: [] },
        },
        dependencies(mutateSignature),
      ),
    ).rejects.toThrow(`function signature resource '${functionPath}' is not hydrated`);

    expect(mutateSignature).not.toHaveBeenCalled();
    expect(submit).not.toHaveBeenCalled();
  });

  it("atomically applies a complete authoritative result", async () => {
    const refreshIndex = vi.spyOn(ProjectService, "getProjectIndex").mockResolvedValue(
      projectIndexSnapshotFixture({
        projectInstanceId,
        projectName: "Project",
        exportTime: "",
        publicationRevision: 1,
        graphs: [
          {
            path: functionPath,
            name: "Compute",
            type: "function",
            revision: 3,
            functionRevision: 3,
            functionSignature: afterSignature,
            functionEditorProjection: authoritativeFunctionProjection,
          },
        ],
        charts: [],
        databases: [],
      }),
    );
    vi.mocked(GraphProjectionService.loadGraph).mockResolvedValue(
      makeGraphEditorSession(
        makeEditorProjectionFixture({
          graphPath: functionPath,
          title: "Committed signature projection",
        }).projection,
      ),
    );
    const committed = result({ status: "complete", expectedGraphPaths: [functionPath] }, true);
    const eventHandler = {
      handle: (payload: { result: ResourceMutationResultDto }) => {
        void projectPublicationCoordinator.submit(payload);
      },
    };
    let graphTitleDuringInvoke: string | undefined;
    let signatureRevisionDuringInvoke: number | undefined;
    const mutateSignature = vi.fn(async () => {
      graphTitleDuringInvoke =
        useGraphProjectionStore.getState().graphEntities[functionPath].nodes["local-node"]?.display
          .title;
      signatureRevisionDuringInvoke =
        useGraphMetaStore.getState().graphs[functionPath].functionRevision;
      eventHandler.handle({ result: committed });
      return committed;
    });

    const outcome = await executeFunctionSignatureMutation(
      {
        functionPath,
        locale: "zh-CN",
        patch: {
          inputs: [{ id: "value", name: "Renamed", dataType: { kind: "Float64" } }],
          outputs: [{ id: "return", name: "Result", dataType: { kind: "Int64" } }],
        },
      },
      dependencies(mutateSignature),
    );

    expect(mutateSignature).toHaveBeenCalledWith(projectInstanceId, functionPath, "zh-CN", {
      resource: { kind: "function", key: functionPath },
      baseRevision: 2,
      operationId,
      payload: { before: beforeSignature, after: afterSignature },
    });
    expect(graphTitleDuringInvoke).toBe("Current graph projection");
    expect(signatureRevisionDuringInvoke).toBe(2);
    expect(outcome).toEqual({ status: "applied", result: committed });
    expect(refreshIndex).toHaveBeenCalledOnce();
    expect(useGraphProjectionStore.getState().graphEntities[functionPath]).toMatchObject({
      nodes: { "local-node": { display: { title: "Committed signature projection" } } },
    });
    expect(useGraphMetaStore.getState().graphs[functionPath]).toMatchObject({
      functionRevision: 3,
      functionSignature: afterSignature,
      functionInputs: authoritativeFunctionProjection.inputs,
      functionOutputs: authoritativeFunctionProjection.outputs,
    });
  });

  it("preserves function authority until an incomplete result receives authoritative projection metadata", async () => {
    const callerPath = "events/Caller.yssbi-event";
    const committed = result(
      {
        status: "incomplete",
        invalidatedGraphPaths: [functionPath, callerPath],
      },
      false,
    );
    const eventHandler = {
      handle: (payload: { result: ResourceMutationResultDto }) => {
        void projectPublicationCoordinator.submit(payload);
      },
    };
    const hydrateGraph = vi.fn(async () => true);
    const beforeMeta = structuredClone(useGraphMetaStore.getState().graphs[functionPath]);
    vi.spyOn(ProjectService, "getProjectIndex").mockResolvedValue(
      projectIndexSnapshotFixture({
        projectInstanceId,
        projectName: "Recovery fixture",

        exportTime: "2026-08-07",
        publicationRevision: 1,
        graphs: [
          {
            path: functionPath,
            name: "Compute",
            type: "function",
            revision: 7,
            functionRevision: 3,
            functionSignature: afterSignature,
            functionEditorProjection: authoritativeFunctionProjection,
          },
        ],
        databases: [],

        charts: [],
      }),
    );

    const outcome = await executeFunctionSignatureMutation(
      {
        functionPath,
        locale: "en-US",
        patch: {
          inputs: [{ id: "value", name: "Renamed", dataType: { kind: "Float64" } }],
          outputs: [{ id: "return", name: "Result", dataType: { kind: "Int64" } }],
        },
      },
      dependencies(
        vi.fn(async () => {
          eventHandler.handle({ result: committed });
          return committed;
        }),
        hydrateGraph,
      ),
    );

    expect(outcome).toEqual({ status: "applied", result: committed });
    expect(useGraphProjectionStore.getState().graphEntities[functionPath]).toMatchObject({
      nodes: { "local-node": { display: { title: "Projected node" } } },
    });
    expect(useGraphMetaStore.getState().graphs[functionPath]).toMatchObject({
      functionRevision: 3,
      functionSignature: afterSignature,
      functionInputs: authoritativeFunctionProjection.inputs,
      functionOutputs: authoritativeFunctionProjection.outputs,
    });
    expect(useGraphMetaStore.getState().graphs[functionPath]).not.toEqual(beforeMeta);
    expect(hydrateGraph).not.toHaveBeenCalled();
    expect(GraphProjectionService.loadGraph).toHaveBeenCalledWith(
      functionPath,
      expect.any(String),
      expect.any(Number),
      projectInstanceId,
    );
    expect(GraphProjectionService.loadGraph).toHaveBeenCalledOnce();
  });

  it("ignores a delayed old-project direct result when identities and publication numbers collide", async () => {
    let resolve!: (value: ResourceMutationResultDto) => void;
    const pendingResult = new Promise<ResourceMutationResultDto>((done) => {
      resolve = done;
    });
    const request = executeFunctionSignatureMutation(
      {
        functionPath,
        locale: "en-US",
        patch: {
          inputs: [{ id: "value", name: "Renamed", dataType: { kind: "Float64" } }],
          outputs: [{ id: "return", name: "Result", dataType: { kind: "Int64" } }],
        },
      },
      dependencies(vi.fn(() => pendingResult)),
    );
    const oldResult = result({ status: "complete", expectedGraphPaths: [functionPath] }, true);

    projectPublicationCoordinator.startProject("00000000-0000-0000-0000-000000000602", 0);
    useGraphProjectionStore.setState({ graphEntities: {} });
    useGraphProjectionStore.getState().replaceProjection(
      functionPath,
      makeEditorProjectionFixture({
        graphPath: functionPath,
        title: "New project",
      }).projection,
    );
    useGraphMetaStore.getState().updateGraph(functionPath, {
      functionRevision: 20,
      functionSignature: afterSignature,
    });
    const beforeGraph = useGraphProjectionStore.getState().graphEntities[functionPath];
    const beforeMeta = useGraphMetaStore.getState().graphs[functionPath];

    await expect(projectPublicationCoordinator.submit({ result: oldResult })).rejects.toMatchObject(
      { code: "stale_project_lifecycle" },
    );
    resolve(oldResult);

    await expect(request).resolves.toEqual({ status: "stale", result: oldResult });
    expect(useGraphProjectionStore.getState().graphEntities[functionPath]).toBe(beforeGraph);
    expect(useGraphMetaStore.getState().graphs[functionPath]).toBe(beforeMeta);
  });

  it("does not install result history independently of the publication coordinator", async () => {
    vi.spyOn(projectPublicationCoordinator, "submit").mockResolvedValue({
      status: "applied",
      affectedGraphPaths: new Set(),
    });
    const committed = result({ status: "complete", expectedGraphPaths: [functionPath] }, true);

    await expect(
      executeFunctionSignatureMutation(
        {
          functionPath,
          locale: "en-US",
          patch: { inputs: [] },
        },
        dependencies(vi.fn(async () => committed)),
      ),
    ).resolves.toMatchObject({ status: "applied" });
  });

  it("rejects malformed correlated results before installing any state", async () => {
    const malformed = result({ status: "complete", expectedGraphPaths: [functionPath] }, true);
    malformed.deltas[0] = {
      ...malformed.deltas[0],
      causedBy: "00000000-0000-0000-0000-000000000502",
    };
    const beforeGraph = useGraphProjectionStore.getState().graphEntities[functionPath];
    const beforeMeta = useGraphMetaStore.getState().graphs[functionPath];
    const hydrateGraph = vi.fn(async () => true);
    const refreshResourceIndex = vi.fn(async () => undefined);

    await expect(
      executeFunctionSignatureMutation(
        {
          functionPath,
          locale: "en-US",
          patch: { inputs: [] },
        },
        dependencies(
          vi.fn(async () => malformed),
          hydrateGraph,
          refreshResourceIndex,
        ),
      ),
    ).rejects.toThrow("resource delta operation correlation is inconsistent");

    expect(useGraphProjectionStore.getState().graphEntities[functionPath]).toBe(beforeGraph);
    expect(useGraphMetaStore.getState().graphs[functionPath]).toBe(beforeMeta);
    expect(refreshResourceIndex).not.toHaveBeenCalled();
    expect(hydrateGraph).not.toHaveBeenCalled();
  });
});
