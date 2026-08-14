import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { useGraphMetaStore } from '@/features/core/dataStore/graphMetaStore';
import { useVariableStore } from '@/features/core/dataStore/variableStore';
import { useHistoryStore } from '@/features/core/history';
import { buildGraphResourceMeta, resourceKey, useResourceStore } from '@/features/core/resource';
import { ResourceMutationCommittedHandler } from '@/features/core/sync/handlers/ProjectMutationEventHandler';
import type {
  FunctionSignatureDto,
  ResourceMutationResultDto,
} from '@/shared/types/dto/editorMutation';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import { GraphProjectionService } from '@/services/nodeSystem/graphProjectionService';
import { ProjectService } from '@/services/project/projectService';
import { getPendingMutation, resetPendingMutations } from './pendingMutationRegistry';
import {
  executeFunctionSignatureMutation,
  resetFunctionSignatureCoordinator,
  type FunctionSignatureCoordinatorDependencies,
} from './functionSignatureCoordinator';
import { projectPublicationCoordinator } from './projectPublicationCoordinator';
import {
  installCoreApplicationTestPorts,
  resetCoreApplicationTestPorts,
} from '@/features/application/testHelpers/coreApplicationPorts';

const functionPath = 'functions/Compute.yssbi-function';
const operationId = '00000000-0000-0000-0000-000000000501';
const projectInstanceId = '00000000-0000-0000-0000-000000000601';

const beforeSignature: FunctionSignatureDto = {
  parameters: [{ id: 'value', name: 'Value', type_name: 'Int64' }],
  return_type: 'Float64',
};
const afterSignature: FunctionSignatureDto = {
  parameters: [{ id: 'value', name: 'Renamed', type_name: 'Float64' }],
  return_type: 'Int64',
};
const authoritativeFunctionProjection = {
  functionRevision: 3,
  inputs: [{ id: 'value', name: 'Observed value', dataType: { kind: 'Float64' as const } }],
  outputs: [{
    id: 'computed',
    name: 'Computed value',
    dataType: { kind: 'Struct' as const, inner: 'RegressionModel' },
  }],
};

vi.mock('@/services/nodeSystem/graphProjectionService', () => ({
  GraphProjectionService: {
    loadGraph: vi.fn(),
    hydrateGraph: vi.fn(),
  },
}));



function installState(): void {
  useGraphDataStore.setState({ graphEntities: {} });
  useResourceStore.getState().clear();
  useResourceStore.getState().upsertResource(
    buildGraphResourceMeta('function', functionPath, 'Compute', { revision: 2 }),
  );
  useGraphDataStore.getState().replaceProjection(
    functionPath,
    makeEditorProjectionFixture({
      graphPath: functionPath,
      sourceRevision: 7,
      title: 'Current graph projection',
    }).projection,
    1,
  );
  useGraphMetaStore.setState({
    graphs: {
      [functionPath]: {
        path: functionPath,
        name: 'Compute',
        type: 'function',
        functionRevision: 2,
        functionSignature: beforeSignature,
        functionInputs: [{ id: 'value', name: 'Value', dataType: { kind: 'Int64' } }],
        functionOutputs: [{ id: 'return', name: 'Result', dataType: { kind: 'Float64' } }],
      },
    },
  });
}

function result(
  projectionStatus: ResourceMutationResultDto['projectionStatus'],
  withReplacement: boolean,
): ResourceMutationResultDto {
  return {
    operationId,
    projectInstanceId,
    publicationRevision: 1,
    moves: [],
    deltas: [{
      resource: { kind: 'function', key: functionPath },
      fromRevision: 2,
      toRevision: 3,
      causedBy: operationId,
      payload: {
        kind: 'function',
        patch: { before: beforeSignature, after: afterSignature },
      },
    }],
    projectionReplacements: withReplacement
      ? [{
          graphPath: functionPath,
          projection: makeEditorProjectionFixture({
            graphPath: functionPath,
            sourceRevision: 7,
            title: 'Committed signature projection',
          }).projection,
          functionEditorProjection: authoritativeFunctionProjection,
        }]
      : [],
    projectionStatus,
    history: { canUndo: true, canRedo: false },
  };
}

function dependencies(
  mutateSignature: FunctionSignatureCoordinatorDependencies['mutateSignature'],
  hydrateGraph = vi.fn(async () => true),
  loadFunctionResources: FunctionSignatureCoordinatorDependencies['loadFunctionResources'] =
    vi.fn(async () => []),
): Partial<FunctionSignatureCoordinatorDependencies> {
  return {
    createOperationId: () => operationId,
    mutateSignature,
    hydrateGraph,
    loadFunctionResources,
  };
}

describe('executeFunctionSignatureMutation', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.clearAllMocks();
    vi.mocked(GraphProjectionService.loadGraph).mockImplementation(async (graphPath) =>
      makeEditorProjectionFixture({ graphPath, sourceRevision: 7 }).projection);
    resetPendingMutations();
    resetFunctionSignatureCoordinator();
    installCoreApplicationTestPorts({
      syncEvents: {
        resourceMutationCommitted: (committed) =>
          projectPublicationCoordinator.submit({ result: committed as ResourceMutationResultDto }),
      },
    });
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
    useHistoryStore.setState({ canUndo: false, canRedo: false, pending: false }, true);
    useVariableStore.setState({ variables: {} });
    installState();
  });

  afterEach(resetCoreApplicationTestPorts);

  it('does not invoke, publish, or mutate when project replacement occurs inside authority read', async () => {
    const authority = useGraphMetaStore.getState();
    vi.spyOn(useGraphMetaStore, 'getState').mockImplementationOnce(() => {
      projectPublicationCoordinator.startProject(
        '00000000-0000-0000-0000-000000000602',
        0,
      );
      return authority;
    });
    const mutateSignature = vi.fn(async () =>
      result({ status: 'complete', expectedGraphPaths: [functionPath] }, true));
    const submit = vi.spyOn(projectPublicationCoordinator, 'submit');
    const beforeMeta = authority.graphs[functionPath];
    const beforeGraph = useGraphDataStore.getState().graphEntities[functionPath];

    await expect(executeFunctionSignatureMutation(
      {
        functionPath,
        locale: 'en-US',
        patch: { inputs: [] },
      },
      dependencies(mutateSignature),
    )).rejects.toMatchObject({ code: 'stale_project_lifecycle' });

    expect(mutateSignature).not.toHaveBeenCalled();
    expect(submit).not.toHaveBeenCalled();
    expect(useGraphMetaStore.getState().graphs[functionPath]).toBe(beforeMeta);
    expect(useGraphDataStore.getState().graphEntities[functionPath]).toBe(beforeGraph);
    expect(getPendingMutation(operationId)).toBeUndefined();
  });

  it('treats a backend stale lifecycle rejection as stale without publication effects', async () => {
    const beforeGraph = useGraphDataStore.getState().graphEntities[functionPath];
    const beforeMeta = useGraphMetaStore.getState().graphs[functionPath];
    const submit = vi.spyOn(projectPublicationCoordinator, 'submit');

    await expect(executeFunctionSignatureMutation(
      {
        functionPath,
        locale: 'en-US',
        patch: { inputs: [] },
      },
      dependencies(vi.fn(async () => {
        throw { code: 'stale_project_lifecycle', message: 'project was replaced' };
      })),
    )).resolves.toEqual({ status: 'stale' });

    expect(submit).not.toHaveBeenCalled();
    expect(useGraphDataStore.getState().graphEntities[functionPath]).toBe(beforeGraph);
    expect(useGraphMetaStore.getState().graphs[functionPath]).toBe(beforeMeta);
    expect(getPendingMutation(operationId)).toBeUndefined();
  });

  it('rejects missing signature authority before invoke or publication effects', async () => {
    useGraphMetaStore.getState().clear();
    const mutateSignature = vi.fn();
    const submit = vi.spyOn(projectPublicationCoordinator, 'submit');

    await expect(executeFunctionSignatureMutation(
      {
        functionPath,
        locale: 'en-US',
        patch: { inputs: [] },
      },
      dependencies(mutateSignature),
    )).rejects.toThrow(`function signature resource '${functionPath}' is not hydrated`);

    expect(mutateSignature).not.toHaveBeenCalled();
    expect(submit).not.toHaveBeenCalled();
    expect(getPendingMutation(operationId)).toBeUndefined();
  });

  it('registers before invoke and atomically applies a complete authoritative result', async () => {
    const committed = result({ status: 'complete', expectedGraphPaths: [functionPath] }, true);
    const eventHandler = new ResourceMutationCommittedHandler();
    let pendingDuringInvoke = false;
    let graphTitleDuringInvoke: string | undefined;
    let signatureRevisionDuringInvoke: number | undefined;
    const mutateSignature = vi.fn(async (_project, _path, _locale, request) => {
      pendingDuringInvoke = getPendingMutation(request.operationId) != null;
      graphTitleDuringInvoke = useGraphDataStore
        .getState()
        .graphEntities[functionPath]
        .nodes['local-node']?.title;
      signatureRevisionDuringInvoke = useGraphMetaStore
        .getState()
        .graphs[functionPath]
        .functionRevision;
      eventHandler.handle({ result: committed });
      return committed;
    });

    const outcome = await executeFunctionSignatureMutation(
      {
        functionPath,
        locale: 'zh-CN',
        patch: {
          inputs: [{ id: 'value', name: 'Renamed', dataType: { kind: 'Float64' } }],
          outputs: [{ id: 'return', name: 'Result', dataType: { kind: 'Int64' } }],
        },
      },
      dependencies(mutateSignature),
    );

    expect(mutateSignature).toHaveBeenCalledWith(projectInstanceId, functionPath, 'zh-CN', {
      resource: { kind: 'function', key: functionPath },
      baseRevision: 2,
      operationId,
      payload: { before: beforeSignature, after: afterSignature },
    });
    expect(pendingDuringInvoke).toBe(true);
    expect(graphTitleDuringInvoke).toBe('Current graph projection');
    expect(signatureRevisionDuringInvoke).toBe(2);
    expect(outcome).toEqual({ status: 'applied', result: committed });
    expect(useGraphDataStore.getState().graphEntities[functionPath]).toMatchObject({
      sourceRevision: 7,
      nodes: { 'local-node': { title: 'Committed signature projection' } },
    });
    expect(useGraphMetaStore.getState().graphs[functionPath]).toMatchObject({
      functionRevision: 3,
      functionSignature: afterSignature,
      functionInputs: authoritativeFunctionProjection.inputs,
      functionOutputs: authoritativeFunctionProjection.outputs,
    });
    expect(useResourceStore.getState().resources[
      resourceKey({ id: functionPath, kind: 'function' })
    ]?.revision).toBe(7);
    expect(useHistoryStore.getState()).toEqual({
      canUndo: true,
      canRedo: false,
      pending: false,
    });
    expect(getPendingMutation(operationId)).toBeUndefined();
  });

  it('preserves function authority until an incomplete result receives authoritative projection metadata', async () => {
    const callerPath = 'events/Caller.yssbi-event';
    const committed = result({
      status: 'incomplete',
      invalidatedGraphPaths: [functionPath, callerPath],
    }, false);
    const eventHandler = new ResourceMutationCommittedHandler();
    const hydrateGraph = vi.fn(async () => true);
    const beforeMeta = structuredClone(useGraphMetaStore.getState().graphs[functionPath]);
    vi.spyOn(ProjectService, 'getProjectIndex').mockResolvedValue({
      projectInstanceId,
      projectName: 'Recovery fixture',

      exportTime: '2026-08-07T00:00:00.000Z',
      publicationRevision: 1,
      history: { canUndo: true, canRedo: false },
      graphs: [{
        path: functionPath,
        name: 'Compute',
        type: 'function',
        revision: 7,
        functionRevision: 3,
        functionSignature: afterSignature,
        functionEditorProjection: authoritativeFunctionProjection,
      }],
      databases: [],
      variables: [],
      worksheets: [],
    });

    const outcome = await executeFunctionSignatureMutation(
      {
        functionPath,
        locale: 'en-US',
        patch: {
          inputs: [{ id: 'value', name: 'Renamed', dataType: { kind: 'Float64' } }],
          outputs: [{ id: 'return', name: 'Result', dataType: { kind: 'Int64' } }],
        },
      },
      dependencies(vi.fn(async () => {
        eventHandler.handle({ result: committed });
        return committed;
      }), hydrateGraph),
    );

    expect(outcome).toEqual({ status: 'applied', result: committed });
    expect(useGraphDataStore.getState().graphEntities[functionPath]).toMatchObject({
      sourceRevision: 7,
      nodes: { 'local-node': { title: 'Projected node' } },
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

    expect(useHistoryStore.getState()).toEqual({
      canUndo: true,
      canRedo: false,
      pending: false,
    });
  });

  it('refreshes canonical function projection and hydrates without local writes on a revision conflict', async () => {
    const beforeGraph = useGraphDataStore.getState().graphEntities[functionPath];
    const hydrateGraph = vi.fn(async () => true);
    const loadFunctionResources = vi.fn(async () => [{
      path: functionPath,
      name: 'Compute',
      type: 'function' as const,
      revision: 7,
      functionRevision: 3,
      functionSignature: afterSignature,
      functionEditorProjection: authoritativeFunctionProjection,
    }]);

    const outcome = await executeFunctionSignatureMutation(
      {
        functionPath,
        locale: 'en-US',
        patch: { inputs: [] },
      },
      dependencies(vi.fn(async () => {
        throw { code: 'function_revision_conflict', message: 'stale signature' };
      }), hydrateGraph, loadFunctionResources),
    );

    expect(outcome).toEqual({ status: 'conflict' });
    expect(useGraphDataStore.getState().graphEntities[functionPath]).toBe(beforeGraph);
    expect(useGraphMetaStore.getState().graphs[functionPath]).toMatchObject({
      functionRevision: 3,
      functionSignature: afterSignature,
      functionInputs: authoritativeFunctionProjection.inputs,
      functionOutputs: authoritativeFunctionProjection.outputs,
    });
    expect(loadFunctionResources).toHaveBeenCalledOnce();
    expect(hydrateGraph).toHaveBeenCalledOnce();
    expect(hydrateGraph).toHaveBeenCalledWith(functionPath, 'en-US');
    expect(useHistoryStore.getState().pending).toBe(false);
    expect(getPendingMutation(operationId)).toBeUndefined();
  });



  it('ignores a delayed old-project direct result when identities and publication numbers collide', async () => {
    let resolve!: (value: ResourceMutationResultDto) => void;
    const pendingResult = new Promise<ResourceMutationResultDto>((done) => {
      resolve = done;
    });
    const request = executeFunctionSignatureMutation(
      {
        functionPath,
        locale: 'en-US',
        patch: {
                  inputs: [{ id: 'value', name: 'Renamed', dataType: { kind: 'Float64' } }],
                  outputs: [{ id: 'return', name: 'Result', dataType: { kind: 'Int64' } }],
                },
      },
      dependencies(vi.fn(() => pendingResult)),
    );
    const oldResult = result({ status: 'complete', expectedGraphPaths: [functionPath] }, true);

    projectPublicationCoordinator.startProject(
      '00000000-0000-0000-0000-000000000602',
      0,
    );
    useHistoryStore.setState({ canUndo: false, canRedo: false, pending: false }, true);
    useGraphDataStore.setState({ graphEntities: {} });
    useGraphDataStore.getState().replaceProjection(
      functionPath,
      makeEditorProjectionFixture({ graphPath: functionPath, sourceRevision: 7, title: 'New project' }).projection,
      1,
    );
    useGraphMetaStore.getState().updateGraph(functionPath, {
      functionRevision: 20,
      functionSignature: afterSignature,
    });
    const beforeGraph = useGraphDataStore.getState().graphEntities[functionPath];
    const beforeMeta = useGraphMetaStore.getState().graphs[functionPath];

    new ResourceMutationCommittedHandler().handle({ result: oldResult });
    resolve(oldResult);

    await expect(request).resolves.toEqual({ status: 'stale', result: oldResult });
    expect(useGraphDataStore.getState().graphEntities[functionPath]).toBe(beforeGraph);
    expect(useGraphMetaStore.getState().graphs[functionPath]).toBe(beforeMeta);
    expect(useHistoryStore.getState()).toEqual({ canUndo: false, canRedo: false, pending: false });
  });

  it('does not install result history independently of the publication coordinator', async () => {
    vi.spyOn(projectPublicationCoordinator, 'submit').mockResolvedValue({
      status: 'applied',
      affectedGraphPaths: new Set(),
    });
    const committed = result({ status: 'complete', expectedGraphPaths: [functionPath] }, true);

    await expect(executeFunctionSignatureMutation(
      {
        functionPath,
        locale: 'en-US',
        patch: { inputs: [] },
      },
      dependencies(vi.fn(async () => committed)),
    )).resolves.toMatchObject({ status: 'applied' });

    expect(useHistoryStore.getState()).toEqual({
      canUndo: false,
      canRedo: false,
      pending: false,
    });
  });

  it('rejects malformed correlated results before installing any state', async () => {
    const malformed = result({ status: 'complete', expectedGraphPaths: [functionPath] }, true);
    malformed.deltas[0] = { ...malformed.deltas[0], causedBy: crypto.randomUUID() };
    const beforeGraph = useGraphDataStore.getState().graphEntities[functionPath];
    const beforeMeta = useGraphMetaStore.getState().graphs[functionPath];
    const hydrateGraph = vi.fn(async () => true);
    const loadFunctionResources = vi.fn(async () => []);

    await expect(executeFunctionSignatureMutation(
      {
        functionPath,
        locale: 'en-US',
        patch: { inputs: [] },
      },
      dependencies(vi.fn(async () => malformed), hydrateGraph, loadFunctionResources),
    )).rejects.toThrow('resource delta operation correlation is inconsistent');

    expect(useGraphDataStore.getState().graphEntities[functionPath]).toBe(beforeGraph);
    expect(useGraphMetaStore.getState().graphs[functionPath]).toBe(beforeMeta);
    expect(loadFunctionResources).not.toHaveBeenCalled();
    expect(hydrateGraph).not.toHaveBeenCalled();
    expect(getPendingMutation(operationId)).toBeUndefined();
  });
});
