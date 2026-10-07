import {
  installGraphProjectionFixture,
  makeEditorProjectionFixture,
  makeGraphEditorSession,
} from "@/tests/helpers/editorProjectionFixtures";
import { projectIndexSnapshotFixture } from "@/tests/helpers/activityPanelFixture";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { buildFileResourceMeta, useResourceStore } from "@/features/core/resource";
import type {
  FunctionSignatureDto,
  ResourceMutationResultDto,
} from "@/shared/types/domain/editorMutation";

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
  parameters: [{ id: "value", name: "Value", type_name: "Numeric" }],
  return_type: "Numeric",
};
const afterSignature: FunctionSignatureDto = {
  parameters: [{ id: "value", name: "Renamed", type_name: "Numeric" }],
  return_type: "Numeric",
};
const authoritativeFunctionProjection = {
  functionRevision: 3,
  inputs: [
    {
      id: "value",
      name: "Observed value",
      dataType: { kind: "Scalar" as const, inner: "Numeric" as const },
    },
  ],
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
  useResourceStore.getState().clear();
  useResourceStore.getState().setSnapshot({
    resources: [buildFileResourceMeta("function_graph", functionPath, "Compute", { revision: 2 })],
  });
  installGraphProjectionFixture(
    functionPath,
    makeEditorProjectionFixture({
      graphPath: functionPath,
      title: "Current graph projection",
    }).projection,
  );
  useResourceStore.setState({
    graphMeta: {
      [functionPath]: {
        type: "function_graph",
        functionRevision: 2,
        functionSignature: beforeSignature,
        functionInputs: [
          { id: "value", name: "Value", dataType: { kind: "Scalar", inner: "Numeric" } },
        ],
        functionOutputs: [
          { id: "return", name: "Result", dataType: { kind: "Scalar", inner: "Numeric" } },
        ],
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
    const beforeMeta = useResourceStore.getState().graphMeta[functionPath];
    const beforeGraph = useResourceStore.getState().graphEntities[functionPath];
    const oldRequest = executeFunctionSignatureMutation(input, overrides);
    resetFunctionSignatureCoordinator();
    const committed = result({ status: "complete", expectedGraphPaths: [functionPath] }, true);
    resolveOld(committed);

    await expect(oldRequest).resolves.toEqual({ status: "stale", result: committed });
    expect(submit).not.toHaveBeenCalled();
    expect(useResourceStore.getState().graphMeta[functionPath]).toBe(beforeMeta);
    expect(useResourceStore.getState().graphEntities[functionPath]).toBe(beforeGraph);
  });

  it("does not invoke, publish, or mutate when project replacement occurs inside authority read", async () => {
    const authority = useResourceStore.getState();
    const read = vi.spyOn(useResourceStore, "getState").mockReturnValue({
      ...authority,
      get graphMeta() {
        projectPublicationCoordinator.startProject("00000000-0000-0000-0000-000000000602", 0);
        return authority.graphMeta;
      },
    });
    const mutateSignature = vi.fn(async () =>
      result({ status: "complete", expectedGraphPaths: [functionPath] }, true),
    );
    const submit = vi.spyOn(projectPublicationCoordinator, "submit");
    const beforeMeta = authority.graphMeta[functionPath];
    const beforeGraph = useResourceStore.getState().graphEntities[functionPath];

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

    read.mockRestore();
    expect(mutateSignature).not.toHaveBeenCalled();
    expect(submit).not.toHaveBeenCalled();
    expect(useResourceStore.getState().graphMeta[functionPath]).toBe(beforeMeta);
    expect(useResourceStore.getState().graphEntities[functionPath]).toBe(beforeGraph);
  });

  it("rejects missing signature authority before invoke or publication effects", async () => {
    useResourceStore.setState({ graphMeta: {} });
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
        eventGraphs: [],
        functionGraphs: [
          {
            path: functionPath,
            name: "Compute",
            type: "function_graph",
            revision: 3,
            functionRevision: 3,
            functionSignature: afterSignature,
            functionEditorProjection: authoritativeFunctionProjection,
          },
        ],
        minds: [],
        docs: [],
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
        useResourceStore.getState().graphEntities[functionPath].nodes["local-node"]?.display.title;
      signatureRevisionDuringInvoke =
        useResourceStore.getState().graphMeta[functionPath].functionRevision;
      eventHandler.handle({ result: committed });
      return committed;
    });

    const outcome = await executeFunctionSignatureMutation(
      {
        functionPath,
        locale: "zh-CN",
        patch: {
          inputs: [
            { id: "value", name: "Renamed", dataType: { kind: "Scalar", inner: "Numeric" } },
          ],
          outputs: [
            { id: "return", name: "Result", dataType: { kind: "Scalar", inner: "Numeric" } },
          ],
        },
      },
      dependencies(mutateSignature),
    );

    expect(mutateSignature).toHaveBeenCalledWith(projectInstanceId, functionPath, {
      resource: { kind: "function", key: functionPath },
      baseRevision: 2,
      operationId,
      payload: { before: beforeSignature, after: afterSignature },
    });
    expect(graphTitleDuringInvoke).toBe("Current graph projection");
    expect(signatureRevisionDuringInvoke).toBe(2);
    expect(outcome).toEqual({ status: "applied", result: committed });
    expect(refreshIndex).toHaveBeenCalledOnce();
    expect(useResourceStore.getState().graphEntities[functionPath]).toMatchObject({
      nodes: { "local-node": { display: { title: "Committed signature projection" } } },
    });
    expect(useResourceStore.getState().graphMeta[functionPath]).toMatchObject({
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
    const beforeMeta = structuredClone(useResourceStore.getState().graphMeta[functionPath]);
    vi.spyOn(ProjectService, "getProjectIndex").mockResolvedValue(
      projectIndexSnapshotFixture({
        projectInstanceId,
        projectName: "Recovery fixture",

        exportTime: "2026-08-07",
        publicationRevision: 1,
        eventGraphs: [],
        functionGraphs: [
          {
            path: functionPath,
            name: "Compute",
            type: "function_graph",
            revision: 7,
            functionRevision: 3,
            functionSignature: afterSignature,
            functionEditorProjection: authoritativeFunctionProjection,
          },
        ],
        databases: [],

        minds: [],
        docs: [],

        charts: [],
      }),
    );

    const outcome = await executeFunctionSignatureMutation(
      {
        functionPath,
        locale: "en-US",
        patch: {
          inputs: [
            { id: "value", name: "Renamed", dataType: { kind: "Scalar", inner: "Numeric" } },
          ],
          outputs: [
            { id: "return", name: "Result", dataType: { kind: "Scalar", inner: "Numeric" } },
          ],
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
    expect(useResourceStore.getState().graphEntities[functionPath]).toMatchObject({
      nodes: { "local-node": { display: { title: "Projected node" } } },
    });
    expect(useResourceStore.getState().graphMeta[functionPath]).toMatchObject({
      functionRevision: 3,
      functionSignature: afterSignature,
      functionInputs: authoritativeFunctionProjection.inputs,
      functionOutputs: authoritativeFunctionProjection.outputs,
    });
    expect(useResourceStore.getState().graphMeta[functionPath]).not.toEqual(beforeMeta);
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
          inputs: [
            { id: "value", name: "Renamed", dataType: { kind: "Scalar", inner: "Numeric" } },
          ],
          outputs: [
            { id: "return", name: "Result", dataType: { kind: "Scalar", inner: "Numeric" } },
          ],
        },
      },
      dependencies(vi.fn(() => pendingResult)),
    );
    const oldResult = result({ status: "complete", expectedGraphPaths: [functionPath] }, true);

    projectPublicationCoordinator.startProject("00000000-0000-0000-0000-000000000602", 0);
    useResourceStore.getState().clear();
    installGraphProjectionFixture(
      functionPath,
      makeEditorProjectionFixture({
        graphPath: functionPath,
        title: "New project",
      }).projection,
    );
    useResourceStore.setState((state) => ({
      graphMeta: {
        ...state.graphMeta,
        [functionPath]: {
          ...state.graphMeta[functionPath],
          functionRevision: 20,
          functionSignature: afterSignature,
        },
      },
    }));
    const beforeGraph = useResourceStore.getState().graphEntities[functionPath];
    const beforeMeta = useResourceStore.getState().graphMeta[functionPath];

    await expect(projectPublicationCoordinator.submit({ result: oldResult })).rejects.toMatchObject(
      { code: "stale_project_lifecycle" },
    );
    resolve(oldResult);

    await expect(request).resolves.toEqual({ status: "stale", result: oldResult });
    expect(useResourceStore.getState().graphEntities[functionPath]).toBe(beforeGraph);
    expect(useResourceStore.getState().graphMeta[functionPath]).toBe(beforeMeta);
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
    const beforeGraph = useResourceStore.getState().graphEntities[functionPath];
    const beforeMeta = useResourceStore.getState().graphMeta[functionPath];
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

    expect(useResourceStore.getState().graphEntities[functionPath]).toBe(beforeGraph);
    expect(useResourceStore.getState().graphMeta[functionPath]).toBe(beforeMeta);
    expect(refreshResourceIndex).not.toHaveBeenCalled();
    expect(hydrateGraph).not.toHaveBeenCalled();
  });
});
