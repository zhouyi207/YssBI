import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { useGraphMetaStore } from '@/features/core/dataStore/graphMetaStore';
import { useVariableStore } from '@/features/core/dataStore/variableStore';
import { useHistoryStore } from '@/features/core/history';
import { ResourceMutationCommittedHandler } from '@/features/core/sync/handlers/ProjectMutationEventHandler';
import type {
  FunctionSignatureDto,
  ResourceMutationResultDto,
} from '@/shared/types/dto/editorMutation';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import { prepareGraphProjectionForPublication } from '@/features/application/editorProjection/graphProjectionCoordinator';
import { getPendingMutation, resetPendingMutations } from './pendingMutationRegistry';
import {
  executeFunctionSignatureMutation,
  resetFunctionSignatureCoordinator,
  type FunctionSignatureCoordinatorDependencies,
} from './functionSignatureCoordinator';
import { projectPublicationCoordinator } from './projectPublicationCoordinator';

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

vi.mock('@/features/application/editorProjection/graphProjectionCoordinator', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/features/application/editorProjection/graphProjectionCoordinator')>()),
  prepareGraphProjectionForPublication: vi.fn(async (graphPath: string) =>
    (await import('@/tests/helpers/editorProjectionFixtures'))
      .makeEditorProjectionFixture({ graphPath }).projection),
}));



function installState(): void {
  useGraphDataStore.setState({ graphEntities: {} });
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
        }]
      : [],
    projectionStatus,
    history: { canUndo: true, canRedo: false },
  };
}

function dependencies(
  mutateSignature: FunctionSignatureCoordinatorDependencies['mutateSignature'],
  hydrateGraph = vi.fn(async () => true),
  loadFunctionResources = vi.fn(async () => []),
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
    resetPendingMutations();
    resetFunctionSignatureCoordinator();
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
    useHistoryStore.setState({ canUndo: false, canRedo: false, pending: false }, true);
    useVariableStore.setState({ variables: {} });
    installState();
  });

  it('registers before invoke and atomically applies a complete authoritative result', async () => {
    const committed = result({ status: 'complete', expectedGraphPaths: [functionPath] }, true);
    const eventHandler = new ResourceMutationCommittedHandler();
    let pendingDuringInvoke = false;
    let graphTitleDuringInvoke: string | undefined;
    let signatureRevisionDuringInvoke: number | undefined;
    const mutateSignature = vi.fn(async (_path, _locale, request) => {
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

    expect(mutateSignature).toHaveBeenCalledWith(functionPath, 'zh-CN', {
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
      functionInputs: [{ id: 'value', name: 'Renamed', dataType: { kind: 'Float64' } }],
      functionOutputs: [{ id: 'return', name: 'Result', dataType: { kind: 'Int64' } }],
    });
    expect(useHistoryStore.getState()).toEqual({
      canUndo: true,
      canRedo: false,
      pending: false,
    });
    expect(getPendingMutation(operationId)).toBeUndefined();
  });

  it('installs signature status and hydrates invalidated graphs for an incomplete result', async () => {
    const callerPath = 'events/Caller.yssbi-event';
    const committed = result({
      status: 'incomplete',
      invalidatedGraphPaths: [functionPath, callerPath],
    }, false);
    const eventHandler = new ResourceMutationCommittedHandler();
    const hydrateGraph = vi.fn(async () => true);

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
      nodes: { 'local-node': { title: 'Current graph projection' } },
    });
    expect(useGraphMetaStore.getState().graphs[functionPath]).toMatchObject({
      functionRevision: 3,
      functionSignature: afterSignature,
    });
    expect(hydrateGraph).not.toHaveBeenCalled();
    expect(prepareGraphProjectionForPublication).toHaveBeenCalledWith(
      functionPath,
      projectInstanceId,
      expect.any(Number),
    );
    expect(prepareGraphProjectionForPublication).toHaveBeenCalledWith(
      callerPath,
      projectInstanceId,
      expect.any(Number),
    );
    expect(useHistoryStore.getState()).toEqual({
      canUndo: true,
      canRedo: false,
      pending: false,
    });
  });

  it('refreshes canonical function state and hydrates without local writes on a revision conflict', async () => {
    const beforeGraph = useGraphDataStore.getState().graphEntities[functionPath];
    const beforeMeta = useGraphMetaStore.getState().graphs[functionPath];
    const hydrateGraph = vi.fn(async () => true);
    const loadFunctionResources = vi.fn(async () => []);

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
    expect(useGraphMetaStore.getState().graphs[functionPath]).toBe(beforeMeta);
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
    )).rejects.toThrow('operation correlation does not match the pending request');

    expect(useGraphDataStore.getState().graphEntities[functionPath]).toBe(beforeGraph);
    expect(useGraphMetaStore.getState().graphs[functionPath]).toBe(beforeMeta);
    expect(loadFunctionResources).not.toHaveBeenCalled();
    expect(hydrateGraph).not.toHaveBeenCalled();
    expect(getPendingMutation(operationId)).toBeUndefined();
  });
});
