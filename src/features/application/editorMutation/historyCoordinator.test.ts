import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { useGraphMetaStore } from '@/features/core/dataStore/graphMetaStore';
import { useVariableStore } from '@/features/core/dataStore/variableStore';
import { useHistoryStore } from '@/features/core/history';
import { ResourceMutationCommittedHandler } from '@/features/core/sync/handlers/ProjectMutationEventHandler';
import type { ResourceMutationResultDto } from '@/shared/types/dto/editorMutation';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import { prepareGraphProjectionForPublication } from '@/features/application/editorProjection/graphProjectionCoordinator';

import { getPendingMutation, resetPendingMutations } from './pendingMutationRegistry';
import {
  executeHistoryMutation,
  resetHistoryCoordinator,
  type HistoryCoordinatorDependencies,
} from './historyCoordinator';
import { projectPublicationCoordinator } from './projectPublicationCoordinator';
import {
  buildGraphResourceMeta,
  markResourceLoaded,
  resourceKey,
  useDocumentStateStore,
  useResourceStore,
} from '@/features/core/resource';

const functionPath = 'functions/Main.yssbi-function';
const eventPath = 'events/Secondary.yssbi-event';
const operationId = '00000000-0000-0000-0000-000000000401';
const projectInstanceId = '00000000-0000-0000-0000-000000000601';
const thresholdVariableId = '00000000-0000-0000-0000-000000000703';



vi.mock('@/features/application/editorProjection/graphProjectionCoordinator', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/features/application/editorProjection/graphProjectionCoordinator')>()),
  prepareGraphProjectionForPublication: vi.fn(async (graphPath: string) =>
    (await import('@/tests/helpers/editorProjectionFixtures'))
      .makeEditorProjectionFixture({ graphPath }).projection),
}));

function installIntVariable(id: string, value: number): void {
  useVariableStore.setState({
    variables: {
      [id]: {
        id,
        name: 'History variable',
        dataType: { kind: 'Int64' },
        dataValue: { kind: 'Int64', value },
        description: '',
        scope: { type: 'global' },
        tags: [],
      },
    },
    revisions: { [id]: 2 },
  });
}

function installProjection(graphPath: string, revision: number, title: string): void {
  useGraphDataStore.getState().replaceProjection(
    graphPath,
    makeEditorProjectionFixture({ graphPath, sourceRevision: revision, title }).projection,
    1,
  );
}

function replacement(graphPath: string, revision: number, title: string) {
  return {
    graphPath,
    projection: makeEditorProjectionFixture({
      graphPath,
      sourceRevision: revision,
      title,
    }).projection,
  };
}

function completeResult(causedBy = operationId): ResourceMutationResultDto {
  return {
    operationId,
    projectInstanceId,
    publicationRevision: 1,
    moves: [],
    deltas: [
      {
        resource: { kind: 'function', key: functionPath },
        fromRevision: 11,
        toRevision: 12,
        causedBy,
        payload: {
          kind: 'function',
          patch: {
            before: { parameters: [], return_type: null },
            after: { parameters: [], return_type: 'float64' },
          },
        },
      },
      {
        resource: { kind: 'graph', key: eventPath },
        fromRevision: 2,
        toRevision: 3,
        causedBy,
        payload: { kind: 'graph', patch: { operations: [] } },
      },
    ],
    projectionReplacements: [
      replacement(functionPath, 12, 'Undone function'),
      replacement(eventPath, 3, 'Undone event'),
    ],
    projectionStatus: {
      status: 'complete',
      expectedGraphPaths: [functionPath, eventPath],
    },
    history: { canUndo: false, canRedo: true },
  };
}

function dependencies(
  invoke: HistoryCoordinatorDependencies['undo'],
  hydrateGraph = vi.fn(async () => true),
): Partial<HistoryCoordinatorDependencies> {
  return {
    createOperationId: () => operationId,
    undo: invoke,
    redo: invoke,
    hydrateGraph,
  };
}

describe('executeHistoryMutation', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.clearAllMocks();
    resetPendingMutations();
    resetHistoryCoordinator();
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
    useGraphDataStore.setState({ graphEntities: {} });
    useGraphMetaStore.setState({
      graphs: {
        [functionPath]: {
          path: functionPath,
          name: 'Main',
          type: 'function',
          functionRevision: 11,
          functionSignature: { parameters: [], return_type: null },
        },
      },
    });
    installProjection(functionPath, 5, 'Current function');
    installProjection(eventPath, 2, 'Current event');
    useHistoryStore.setState({ canUndo: true, canRedo: false, pending: false }, true);
    useVariableStore.setState({ variables: {} });
    useResourceStore.getState().clear();
    useDocumentStateStore.getState().clear();
    useResourceStore.getState().setSnapshot({
      resources: [buildGraphResourceMeta('function', functionPath, 'Main')],
      graphOrder: [functionPath],
    });
    markResourceLoaded({ id: functionPath, kind: 'function' });
  });

  it.each(['undo', 'redo'] as const)(
    'installs the authoritative destination name for a history %s rename',
    async (direction) => {
      const restoredPath = 'functions/Restored.yssbi-function';
    const result: ResourceMutationResultDto = {
      operationId,
      projectInstanceId,
      publicationRevision: 1,
      moves: [{
        from: functionPath,
        to: restoredPath,
        kind: 'function',
        name: 'Restored Function',
      }],
      deltas: [{
        resource: { kind: 'graph', key: restoredPath },
        fromRevision: 0,
        toRevision: 1,
        causedBy: operationId,
        payload: {
          kind: 'graph_resource_move',
          patch: { from: functionPath, to: restoredPath },
        },
      }],
      projectionReplacements: [],
      projectionStatus: { status: 'incomplete', invalidatedGraphPaths: [restoredPath] },
      history: { canUndo: false, canRedo: true },
    };

      await expect(executeHistoryMutation(
        { direction, graphPath: functionPath, locale: 'en-US' },
        dependencies(vi.fn(async () => result)),
      )).resolves.toMatchObject({ status: 'applied' });

      expect(useGraphMetaStore.getState().graphs[restoredPath]).toMatchObject({
      path: restoredPath,
      name: 'Restored Function',
      type: 'function',
    });
    expect(useResourceStore.getState().resources[
      resourceKey({ id: restoredPath, kind: 'function' })
    ]).toMatchObject({ id: restoredPath, name: 'Restored Function', loaded: true });
      expect(useGraphDataStore.getState().graphEntities[restoredPath]?.basis.graphPath)
        .toBe(restoredPath);
    },
  );

  it('does not install result history independently of the publication coordinator', async () => {
    vi.spyOn(projectPublicationCoordinator, 'submit').mockResolvedValue({
      status: 'applied',
      affectedGraphPaths: new Set(),
    });

    await expect(executeHistoryMutation(
      { direction: 'undo', graphPath: functionPath, locale: 'en-US' },
      dependencies(vi.fn(async () => completeResult())),
    )).resolves.toMatchObject({ status: 'applied' });

    expect(useHistoryStore.getState()).toEqual({
      canUndo: true,
      canRedo: false,
      pending: false,
    });
  });

  it.each(['undo', 'redo'] as const)(
    'calls Rust %s with the current resource revision and a registered operation ID',
    async (direction) => {
      let pendingDuringInvoke = false;
      const invoke = vi.fn(async (_locale, request) => {
        pendingDuringInvoke = useHistoryStore.getState().pending
          && getPendingMutation(request.operationId) != null;
        return completeResult();
      });
      const outcome = await executeHistoryMutation(
        { direction, graphPath: functionPath, locale: 'zh-CN' },
        dependencies(invoke),
      );

      expect(invoke).toHaveBeenCalledWith('zh-CN', {
        resource: { kind: 'graph', key: functionPath },
        baseRevision: 5,
        operationId,
        payload: {},
      });
      expect(pendingDuringInvoke).toBe(true);
      expect(useGraphDataStore.getState().graphEntities[functionPath]).toMatchObject({
        sourceRevision: 12,
        nodes: { 'local-node': { title: 'Undone function' } },
      });
      expect(useGraphDataStore.getState().graphEntities[eventPath]).toMatchObject({
        sourceRevision: 3,
        nodes: { 'local-node': { title: 'Undone event' } },
      });
      expect(outcome.status).toBe('applied');
      expect(useHistoryStore.getState()).toEqual({
        canUndo: false,
        canRedo: true,
        pending: false,
      });
      expect(getPendingMutation(operationId)).toBeUndefined();
    },
  );

  it('suppresses the correlated incomplete event and applies the returned IPC result once', async () => {
    const result: ResourceMutationResultDto = {
      operationId,
      projectInstanceId,
      publicationRevision: 1,
      moves: [],
      deltas: [{
        resource: { kind: 'function', key: functionPath },
        fromRevision: 11,
        toRevision: 12,
        causedBy: operationId,
        payload: {
          kind: 'function',
          patch: {
            before: { parameters: [], return_type: null },
            after: { parameters: [], return_type: 'float64' },
          },
        },
      }],
      projectionReplacements: [],
      projectionStatus: {
        status: 'incomplete',
        invalidatedGraphPaths: [functionPath],
      },
      history: { canUndo: false, canRedo: true },
    };
    const hydrateGraph = vi.fn(async () => true);
    const eventHandler = new ResourceMutationCommittedHandler();
    let statusAfterEvent: ReturnType<typeof useHistoryStore.getState> | undefined;
    const invoke = vi.fn(async () => {
      eventHandler.handle({ result });
      statusAfterEvent = useHistoryStore.getState();
      return result;
    });

    const outcome = await executeHistoryMutation(
      { direction: 'undo', graphPath: functionPath, locale: 'en-US' },
      dependencies(invoke, hydrateGraph),
    );

    expect(invoke).toHaveBeenCalledWith('en-US', {
      resource: { kind: 'graph', key: functionPath },
      baseRevision: 5,
      operationId,
      payload: {},
    });
    expect(statusAfterEvent).toMatchObject({ canUndo: true, canRedo: false, pending: true });
    expect(outcome).toEqual({ status: 'applied', result });
    expect(useGraphDataStore.getState().graphEntities[functionPath].sourceRevision).toBe(5);
    expect(useHistoryStore.getState()).toEqual({
      canUndo: false,
      canRedo: true,
      pending: false,
    });
    expect(hydrateGraph).not.toHaveBeenCalled();
    expect(prepareGraphProjectionForPublication).toHaveBeenCalledWith(
      functionPath,
      projectInstanceId,
      expect.any(Number),
    );
  });

  it('validates every correlated delta before atomically replacing any projection', async () => {
    const invalid = completeResult('00000000-0000-0000-0000-000000000499');
    const hydrateGraph = vi.fn(async () => true);
    const beforeFunction = useGraphDataStore.getState().graphEntities[functionPath];
    const beforeEvent = useGraphDataStore.getState().graphEntities[eventPath];

    await expect(executeHistoryMutation(
      { direction: 'undo', graphPath: functionPath, locale: 'en-US' },
      dependencies(vi.fn(async () => invalid), hydrateGraph),
    )).rejects.toThrow(/history result/i);

    expect(useGraphDataStore.getState().graphEntities[functionPath]).toBe(beforeFunction);
    expect(useGraphDataStore.getState().graphEntities[eventPath]).toBe(beforeEvent);
    expect(hydrateGraph).not.toHaveBeenCalled();
    expect(prepareGraphProjectionForPublication).not.toHaveBeenCalled();
    expect(useHistoryStore.getState().pending).toBe(false);
  });

  it('accepts a correlated transaction that does not modify the concurrency anchor', async () => {
    installIntVariable(thresholdVariableId, 10);
    const result: ResourceMutationResultDto = {
      operationId,
      projectInstanceId,
      publicationRevision: 1,
      moves: [],
      deltas: [{
        resource: { kind: 'variable', key: `variables/${thresholdVariableId}` },
        fromRevision: 2,
        toRevision: 3,
        causedBy: operationId,
        payload: {
          kind: 'variable',
          patch: {
            before: {
              id: thresholdVariableId,
              name: 'History variable',
              dataType: 'Int64',
              dataValue: { Int64: 10 },
              description: '',
              scope: { type: 'global' },
              tags: [],
            },
            after: {
              id: thresholdVariableId,
              name: 'History variable',
              dataType: 'Int64',
              dataValue: { Int64: 5 },
              description: '',
              scope: { type: 'global' },
              tags: [],
            },
          },
        },
      }],
      projectionReplacements: [],
      projectionStatus: { status: 'complete', expectedGraphPaths: [] },
      history: { canUndo: false, canRedo: true },
    };

    const outcome = await executeHistoryMutation(
      { direction: 'undo', graphPath: functionPath, locale: 'en-US' },
      dependencies(vi.fn(async () => result)),
    );

    expect(outcome).toEqual({ status: 'applied', result });
    expect(useGraphDataStore.getState().graphEntities[functionPath].sourceRevision).toBe(5);
    expect(useHistoryStore.getState()).toEqual({
      canUndo: false,
      canRedo: true,
      pending: false,
    });
  });

  it('installs valid partial replacements and hydrates every incomplete invalidation', async () => {
    const invalidatedPath = 'events/Invalidated.yssbi-event';
    const result = completeResult();
    result.deltas = result.deltas.slice(0, 1);
    result.projectionReplacements = result.projectionReplacements.slice(0, 1);
    result.projectionStatus = {
      status: 'incomplete',
      invalidatedGraphPaths: [eventPath, invalidatedPath],
    };
    const hydrateGraph = vi.fn(async () => true);

    await executeHistoryMutation(
      { direction: 'undo', graphPath: functionPath, locale: 'en-US' },
      dependencies(vi.fn(async () => result), hydrateGraph),
    );

    expect(useGraphDataStore.getState().graphEntities[functionPath].sourceRevision).toBe(12);
    expect(useGraphDataStore.getState().graphEntities[eventPath].sourceRevision).toBe(2);
    expect(hydrateGraph).not.toHaveBeenCalled();
    expect(prepareGraphProjectionForPublication).toHaveBeenCalledTimes(3);
    expect(prepareGraphProjectionForPublication).toHaveBeenCalledWith(
      functionPath,
      projectInstanceId,
      expect.any(Number),
    );
    expect(prepareGraphProjectionForPublication).toHaveBeenCalledWith(
      eventPath,
      projectInstanceId,
      expect.any(Number),
    );
    expect(prepareGraphProjectionForPublication).toHaveBeenCalledWith(
      invalidatedPath,
      projectInstanceId,
      expect.any(Number),
    );
  });



  it('changes no committed entities on conflict and hydrates the anchor graph', async () => {
    const hydrateGraph = vi.fn(async () => true);
    const before = useGraphDataStore.getState().graphEntities;

    const outcome = await executeHistoryMutation(
      { direction: 'undo', graphPath: functionPath, locale: 'en-US' },
      dependencies(vi.fn(async () => {
        throw { code: 'history_revision_conflict', message: 'stale anchor' };
      }), hydrateGraph),
    );

    expect(outcome).toEqual({ status: 'conflict' });
    expect(useGraphDataStore.getState().graphEntities).toBe(before);
    expect(hydrateGraph).toHaveBeenCalledOnce();
    expect(hydrateGraph).toHaveBeenCalledWith(functionPath, 'en-US');
    expect(useHistoryStore.getState().pending).toBe(false);
    expect(getPendingMutation(operationId)).toBeUndefined();
  });

  it('ignores a delayed old-project direct result when identities and publication numbers collide', async () => {
    let resolve!: (result: ResourceMutationResultDto) => void;
    const pendingResult = new Promise<ResourceMutationResultDto>((done) => {
      resolve = done;
    });
    const request = executeHistoryMutation(
      { direction: 'undo', graphPath: functionPath, locale: 'en-US' },
      dependencies(vi.fn(() => pendingResult)),
    );

    expect(useHistoryStore.getState().pending).toBe(true);
    projectPublicationCoordinator.startProject(
      '00000000-0000-0000-0000-000000000602',
      0,
    );
    useHistoryStore.setState({ canUndo: false, canRedo: false, pending: true }, true);
    expect(useHistoryStore.getState()).toEqual({
      canUndo: false,
      canRedo: false,
      pending: true,
    });
    expect(getPendingMutation(operationId)).toBeDefined();

    resolve(completeResult());
    await expect(request).resolves.toEqual({ status: 'stale' });
    expect(useGraphDataStore.getState().graphEntities[functionPath].sourceRevision).toBe(5);
    expect(useHistoryStore.getState()).toEqual({
      canUndo: false,
      canRedo: false,
      pending: false,
    });
  });
});
