import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  registerPendingMutation,
  resetPendingMutations,
} from '@/features/application/editorMutation/pendingMutationRegistry';
import * as pendingMutationRegistry from '@/features/application/editorMutation/pendingMutationRegistry';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import { invalidateGraphProjection } from '@/features/application/editorProjection/graphProjectionCoordinator';
import { useDatabaseStore } from '@/features/core/dataStore/databaseStore';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { useHistoryStore } from '@/features/core/history';
import { useNodeCatalogStore } from '@/features/core/nodeCatalog/nodeCatalogStore';
import {
  markResourceLoaded,
  useDocumentStateStore,
  useResourceStore,
} from '@/features/core/resource';
import { ProjectService } from '@/services/project/projectService';
import { GraphProjectionService } from '@/services/nodeSystem/graphProjectionService';
import type { ResourceMutationResultDto } from '@/shared/types/dto/editorMutation';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import { logger } from '@/utils/appLogger';
import {
  GraphDeltaHandler,
  ResourceMutationCommittedHandler,
} from './ProjectMutationEventHandler';

vi.mock('@/features/application/editorProjection/graphProjectionCoordinator', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/features/application/editorProjection/graphProjectionCoordinator')>()),
  invalidateGraphProjection: vi.fn(async () => true),
}));

vi.mock('@/services/project/projectService', () => ({
  ProjectService: {
    getProjectIndex: vi.fn(),
  },
}));

vi.mock('@/services/nodeSystem/graphProjectionService', () => ({
  GraphProjectionService: {
    loadGraph: vi.fn(),
    hydrateGraph: vi.fn(),
  },
}));

const graphPath = 'events/Main.yssbi-event';
const projectInstanceId = '00000000-0000-0000-0000-000000000601';
const operationId = '00000000-0000-0000-0000-000000000401';
const functionPath = 'functions/Forecast.yssbi-function';

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

function resourceResult(publicationRevision = 1): ResourceMutationResultDto {
  return {
    operationId,
    projectInstanceId,
    publicationRevision,
    moves: [],
    deltas: [{
      resource: { kind: 'graph', key: graphPath },
      fromRevision: 1,
      toRevision: 2,
      causedBy: operationId,
      payload: { kind: 'graph', patch: { operations: [] } },
    }],
    projectionReplacements: [{
      graphPath,
      projection: makeEditorProjectionFixture({
        graphPath,
        sourceRevision: 2,
        nodeId: '00000000-0000-0000-0000-000000000603',
        title: 'Committed',
      }).projection,
    }],
    projectionStatus: { status: 'complete', expectedGraphPaths: [graphPath] },
    history: { canUndo: true, canRedo: false },
  };
}

function functionResult(functionRevision: number) {
  const projection = makeEditorProjectionFixture({
    graphPath: functionPath,
    sourceRevision: 1,
    nodeId: '00000000-0000-0000-0000-000000000604',
    title: 'Function committed',
  }).projection;
  return {
    operationId,
    projectInstanceId,
    publicationRevision: 1,
    moves: [],
    deltas: [{
      resource: { kind: 'function', key: functionPath },
      fromRevision: 0,
      toRevision: 1,
      causedBy: operationId,
      payload: {
        kind: 'function',
        patch: {
          before: { parameters: [], return_type: null },
          after: { parameters: [], return_type: 'Array<String>' },
        },
      },
    }],
    projectionReplacements: [{
      graphPath: functionPath,
      projection,
      functionEditorProjection: {
        functionRevision,
        inputs: [],
        outputs: [{
          id: 'return',
          name: 'Array<String>',
          dataType: { kind: 'Array', inner: { kind: 'String' } },
        }],
      },
    }],
    projectionStatus: { status: 'complete', expectedGraphPaths: [functionPath] },
    history: { canUndo: true, canRedo: false },
  };
}

function databaseResult(publicationRevision = 1): ResourceMutationResultDto {
  const before = {
    id: 'sales',
    engine: { duckDb: { path: 'database/project.duckdb', table: 'sales' } },
    schemaVersion: 1,
    required: false,
    name: 'Before',
  };
  return {
    operationId,
    projectInstanceId,
    publicationRevision,
    moves: [],
    deltas: [{
      resource: { kind: 'database', key: 'opaque database resource path' },
      fromRevision: 4,
      toRevision: 5,
      causedBy: operationId,
      payload: { kind: 'database', patch: { before, after: { ...before, name: 'After' } } },
    }],
    projectionReplacements: [],
    projectionStatus: { status: 'complete', expectedGraphPaths: [] },
    history: { canUndo: false, canRedo: false },
  };
}

function emptyResult(
  publicationRevision: number,
  history = { canUndo: true, canRedo: false },
): ResourceMutationResultDto {
  return {
    operationId,
    projectInstanceId,
    publicationRevision,
    moves: [],
    deltas: [],
    projectionReplacements: [],
    projectionStatus: { status: 'complete', expectedGraphPaths: [] },
    history,
  };
}

function recoveryIndex(publicationRevision: number) {
  return {
    projectInstanceId,
    publicationRevision,
    history: { canUndo: false, canRedo: false },
    projectName: 'Test',
    graphs: [],
    variables: [],
    worksheets: [],
    databases: [],
    exportTime: '',
    appVersion: '0.2.7',
  };
}

describe('Project mutation event synchronization', () => {
  beforeEach(() => {
    projectPublicationCoordinator.cancelProject();
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
    vi.restoreAllMocks();
    vi.clearAllMocks();
    resetPendingMutations();
    useGraphDataStore.setState({ graphEntities: {} });
    useDatabaseStore.setState({ databases: {}, revisions: {} });
    useNodeCatalogStore.getState().clear();
    useHistoryStore.setState({ canUndo: false, canRedo: false, pending: false }, true);
  });

  it('suppresses only the GraphDelta echo whose operation ID is pending', () => {
    registerPendingMutation({ operationId, graphPath, baseRevision: 1 });

    new GraphDeltaHandler().handle({
      projectInstanceId,
      delta: {
        graphPath,
        fromRevision: 1,
        toRevision: 2,
        causedBy: operationId,
        payload: { operations: [] },
      },
    });

    expect(invalidateGraphProjection).not.toHaveBeenCalled();
  });

  it('requests projection invalidation for a newer external GraphDelta', () => {
    useGraphDataStore.getState().replaceProjection(
      graphPath,
      makeEditorProjectionFixture({ graphPath, sourceRevision: 1 }).projection,
      1,
    );

    new GraphDeltaHandler().handle({
      projectInstanceId,
      delta: {
        graphPath,
        fromRevision: 1,
        toRevision: 2,
        causedBy: null,
        payload: { operations: [] },
      },
    });

    expect(invalidateGraphProjection).toHaveBeenCalledOnce();
    expect(invalidateGraphProjection).toHaveBeenCalledWith(graphPath);
  });

  it('lets downstream invalidation reject a pending response after project replacement', async () => {
    const current = makeEditorProjectionFixture({ graphPath, sourceRevision: 1, title: 'Current' });
    const replacement = makeEditorProjectionFixture({
      graphPath,
      sourceRevision: 2,
      title: 'Old project response',
    });
    const pending = deferred<typeof replacement.projection>();
    const actualCoordinator = await vi.importActual<
      typeof import('@/features/application/editorProjection/graphProjectionCoordinator')
    >('@/features/application/editorProjection/graphProjectionCoordinator');
    let invalidation!: Promise<boolean>;
    vi.mocked(invalidateGraphProjection).mockImplementationOnce((path) => {
      invalidation = actualCoordinator.invalidateGraphProjection(path);
      return invalidation;
    });
    useGraphDataStore.getState().replaceProjection(graphPath, current.projection, 1);
    markResourceLoaded({ id: graphPath, kind: 'event' });
    vi.mocked(GraphProjectionService.hydrateGraph).mockReturnValueOnce(pending.promise);

    new GraphDeltaHandler().handle({
      projectInstanceId,
      delta: {
        graphPath,
        fromRevision: 1,
        toRevision: 2,
        causedBy: null,
        payload: { operations: [] },
      },
    });
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
    pending.resolve(replacement.projection);

    await expect(invalidation).resolves.toBe(false);
    expect(useGraphDataStore.getState().graphEntities[graphPath]).toMatchObject({
      sourceRevision: 1,
      nodes: { 'local-node': { title: 'Current' } },
    });
  });

  it('gives a malformed GraphDelta path zero pending, graph, or resource store effects', () => {
    const pendingLookup = vi.spyOn(pendingMutationRegistry, 'getPendingMutation');
    const graphStateRead = vi.spyOn(useGraphDataStore, 'getState');
    const resourceWrite = vi.spyOn(useResourceStore, 'setState');
    const documentWrite = vi.spyOn(useDocumentStateStore, 'setState');

    new GraphDeltaHandler().handle({
      projectInstanceId,
      delta: {
        graphPath: 'malformed path',
        fromRevision: 1,
        toRevision: 2,
        causedBy: null,
        payload: { operations: [] },
      },
    } as never);

    expect(pendingLookup).not.toHaveBeenCalled();
    expect(graphStateRead).not.toHaveBeenCalled();
    expect(resourceWrite).not.toHaveBeenCalled();
    expect(documentWrite).not.toHaveBeenCalled();
    expect(invalidateGraphProjection).not.toHaveBeenCalled();
  });

  it('ignores GraphDelta revisions already represented by the projection', () => {
    useGraphDataStore.getState().replaceProjection(
      graphPath,
      makeEditorProjectionFixture({ graphPath, sourceRevision: 2 }).projection,
      1,
    );

    new GraphDeltaHandler().handle({
      projectInstanceId,
      delta: {
        graphPath,
        fromRevision: 1,
        toRevision: 2,
        causedBy: null,
        payload: { operations: [] },
      },
    });

    expect(invalidateGraphProjection).not.toHaveBeenCalled();
  });

  it('rejects project replacement between graph state read and invalidation', () => {
    const originalGetState = useGraphDataStore.getState;
    vi.spyOn(useGraphDataStore, 'getState').mockImplementationOnce(() => {
      projectPublicationCoordinator.startProject(projectInstanceId, 0);
      return originalGetState();
    });

    new GraphDeltaHandler().handle({
      projectInstanceId,
      delta: {
        graphPath,
        fromRevision: 1,
        toRevision: 2,
        causedBy: null,
        payload: { operations: [] },
      },
    });

    expect(invalidateGraphProjection).not.toHaveBeenCalled();
  });

  it('rejects stale GraphDelta identity before pending lookup or graph store reads', () => {
    projectPublicationCoordinator.startProject('00000000-0000-0000-0000-000000000602', 0);
    const pendingLookup = vi.spyOn(pendingMutationRegistry, 'getPendingMutation');
    const graphStateRead = vi.spyOn(useGraphDataStore, 'getState');

    new GraphDeltaHandler().handle({
      projectInstanceId,
      delta: {
        graphPath,
        fromRevision: 1,
        toRevision: 2,
        causedBy: operationId,
        payload: { operations: [] },
      },
    });

    expect(pendingLookup).not.toHaveBeenCalled();
    expect(graphStateRead).not.toHaveBeenCalled();
    expect(invalidateGraphProjection).not.toHaveBeenCalled();
  });

  it('rejects incoherent function replacement revisions before any store effect', () => {
    const submit = vi.spyOn(projectPublicationCoordinator, 'submit').mockResolvedValue({
      status: 'applied',
      affectedGraphPaths: new Set(),
    });
    const graphWrite = vi.spyOn(useGraphDataStore, 'setState');
    const databaseWrite = vi.spyOn(useDatabaseStore, 'setState');
    const historyWrite = vi.spyOn(useHistoryStore, 'setState');

    new ResourceMutationCommittedHandler().handle({ result: functionResult(2) } as never);

    expect(submit).not.toHaveBeenCalled();
    expect(graphWrite).not.toHaveBeenCalled();
    expect(databaseWrite).not.toHaveBeenCalled();
    expect(historyWrite).not.toHaveBeenCalled();
  });

  it('ignores stale project events before coordinator submission', () => {
    projectPublicationCoordinator.startProject('00000000-0000-0000-0000-000000000602', 0);
    const submit = vi.spyOn(projectPublicationCoordinator, 'submit').mockResolvedValue({
      status: 'applied',
      affectedGraphPaths: new Set(),
    });
    registerPendingMutation({ operationId, graphPath, baseRevision: 1 });

    new ResourceMutationCommittedHandler().handle({ result: resourceResult() });

    expect(submit).not.toHaveBeenCalled();
  });

  it('delivers a committed resource result even when its operation ID is pending', () => {
    const submit = vi.spyOn(projectPublicationCoordinator, 'submit').mockResolvedValue({
      status: 'applied',
      affectedGraphPaths: new Set(),
    });
    const result = resourceResult();
    registerPendingMutation({ operationId, graphPath, baseRevision: 1 });

    new ResourceMutationCommittedHandler().handle({ result });

    expect(submit).toHaveBeenCalledOnce();
    expect(submit).toHaveBeenCalledWith({ result });
  });

  it('applies a database publication and then advances the canonical Catalog watermark', async () => {
    useDatabaseStore.setState({
      databases: {
        sales: {
          id: 'sales',
          name: 'Before',
          resourcePath: 'opaque database resource path',
          engine: { duckDb: { path: 'database/project.duckdb', table: 'sales' } },
          schemaVersion: 1,
          required: false,
        },
      },
      revisions: { sales: 4 },
    });
    const observe = vi.spyOn(useNodeCatalogStore.getState(), 'observeResourcePublication');

    new ResourceMutationCommittedHandler().handle({ result: databaseResult(1) });

    await vi.waitFor(() => expect(useDatabaseStore.getState().revisions.sales).toBe(5));
    expect(useDatabaseStore.getState().databases.sales?.name).toBe('After');
    expect(observe).toHaveBeenCalledWith(projectInstanceId, 1);
  });

  it('advances the Catalog refresh watermark only after the resource event settles', async () => {
    let settle!: () => void;
    vi.spyOn(projectPublicationCoordinator, 'submit').mockReturnValue(new Promise((resolve) => {
      settle = () => resolve({ status: 'applied', affectedGraphPaths: new Set() });
    }));
    const observe = vi.spyOn(useNodeCatalogStore.getState(), 'observeResourcePublication');

    new ResourceMutationCommittedHandler().handle({ result: resourceResult(3) });
    expect(observe).not.toHaveBeenCalled();

    settle();
    await Promise.resolve();
    await Promise.resolve();

    expect(observe).toHaveBeenCalledOnce();
    expect(observe).toHaveBeenCalledWith(projectInstanceId, 3);
  });

  it('does not advance the Catalog after project replacement while submission settles', async () => {
    const settlement = deferred<{
      status: 'applied';
      affectedGraphPaths: ReadonlySet<string>;
    }>();
    vi.spyOn(projectPublicationCoordinator, 'submit').mockReturnValue(settlement.promise);
    const observe = vi.spyOn(useNodeCatalogStore.getState(), 'observeResourcePublication');

    new ResourceMutationCommittedHandler().handle({ result: resourceResult(3) });
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
    settlement.resolve({ status: 'applied', affectedGraphPaths: new Set() });
    await settlement.promise;
    await Promise.resolve();

    expect(observe).not.toHaveBeenCalled();
  });

  it('delivers matching direct and event receipts to coordinator-owned deduplication', () => {
    const submit = vi.spyOn(projectPublicationCoordinator, 'submit').mockResolvedValue({
      status: 'applied',
      affectedGraphPaths: new Set(),
    });
    const result = resourceResult();
    const handler = new ResourceMutationCommittedHandler();

    handler.handle({ result });
    handler.handle({ result: structuredClone(result) });

    expect(submit).toHaveBeenCalledTimes(2);
  });

  it('gives malformed ResourceMutationCommitted payloads zero store effects', () => {
    const submit = vi.spyOn(projectPublicationCoordinator, 'submit').mockResolvedValue({
      status: 'applied',
      affectedGraphPaths: new Set(),
    });
    const graphWrite = vi.spyOn(useGraphDataStore, 'setState');
    const databaseWrite = vi.spyOn(useDatabaseStore, 'setState');
    const historyWrite = vi.spyOn(useHistoryStore, 'setState');
    const catalogRead = vi.spyOn(useNodeCatalogStore, 'getState');

    new ResourceMutationCommittedHandler().handle({
      result: { ...resourceResult(), publicationRevision: 0 },
    } as never);

    expect(submit).not.toHaveBeenCalled();
    expect(graphWrite).not.toHaveBeenCalled();
    expect(databaseWrite).not.toHaveBeenCalled();
    expect(historyWrite).not.toHaveBeenCalled();
    expect(catalogRead).not.toHaveBeenCalled();
  });

  it('ignores malformed event envelopes without coordinator submission', () => {
    const submit = vi.spyOn(projectPublicationCoordinator, 'submit').mockResolvedValue({
      status: 'applied',
      affectedGraphPaths: new Set(),
    });
    const handler = new ResourceMutationCommittedHandler();

    handler.handle({ result: null as never });
    handler.handle({ result: 'bad' as never });

    expect(submit).not.toHaveBeenCalled();
  });

  it('logs asynchronous coordinator rejection at the event boundary', async () => {
    const logError = vi.spyOn(logger.sys, 'error').mockImplementation(() => undefined);
    const submit = vi.spyOn(projectPublicationCoordinator, 'submit').mockRejectedValueOnce(
      new Error('publication failed'),
    );

    new ResourceMutationCommittedHandler().handle({ result: resourceResult() });
    await Promise.resolve();
    await Promise.resolve();

    expect(submit).toHaveBeenCalledOnce();
    expect(logError).toHaveBeenCalledWith(
      'Resource publication event failed: publication failed',
      'ResourceMutationCommittedHandler',
    );
  });

  it.each(['event-first', 'direct-first'] as const)(
    '%s matching deliveries settle through one coordinator commit',
    async (order) => {
      projectPublicationCoordinator.startProject(projectInstanceId, 0);
      const result = emptyResult(1);
      const submissions: Promise<unknown>[] = [];
      const originalSubmit = projectPublicationCoordinator.submit.bind(projectPublicationCoordinator);
      const submit = vi.spyOn(projectPublicationCoordinator, 'submit').mockImplementation((input) => {
        const promise = originalSubmit(input);
        submissions.push(promise);
        return promise;
      });
      const setHistory = vi.spyOn(useHistoryStore, 'setState');
      const handler = new ResourceMutationCommittedHandler();

      if (order === 'event-first') {
        handler.handle({ result: structuredClone(result) });
        projectPublicationCoordinator.submit({ result });
      } else {
        projectPublicationCoordinator.submit({ result });
        handler.handle({ result: structuredClone(result) });
      }

      await expect(Promise.all(submissions)).resolves.toMatchObject([
        { status: 'applied' },
        { status: 'duplicate' },
      ]);
      expect(submit).toHaveBeenCalledTimes(2);
      expect(setHistory).toHaveBeenCalledOnce();
      expect(useHistoryStore.getState()).toEqual({
        canUndo: true,
        canRedo: false,
        pending: false,
      });
    },
  );

  it('keeps reverse event arrival revision ordered through the real handler', async () => {
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
    let resolveSnapshot!: (value: ReturnType<typeof recoveryIndex>) => void;
    vi.mocked(ProjectService.getProjectIndex).mockReturnValue(new Promise((resolve) => {
      resolveSnapshot = resolve;
    }));
    const setHistory = vi.spyOn(useHistoryStore, 'setState');
    const handler = new ResourceMutationCommittedHandler();

    handler.handle({ result: emptyResult(2, { canUndo: false, canRedo: true }) });
    await vi.waitFor(() => expect(ProjectService.getProjectIndex).toHaveBeenCalledOnce());
    handler.handle({ result: emptyResult(1, { canUndo: true, canRedo: false }) });
    resolveSnapshot(recoveryIndex(0));

    await vi.waitFor(() => {
      expect(projectPublicationCoordinator.getSnapshotForTests().appliedRevision).toBe(2);
    });
    expect(setHistory.mock.calls.map(([update]) => update)).toEqual([
      { canUndo: true, canRedo: false },
      { canUndo: false, canRedo: true },
    ]);
  });

  it('does not perform fallback graph hydration after coordinator recovery failure', async () => {
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
    vi.spyOn(logger.sys, 'error').mockImplementation(() => undefined);
    vi.mocked(ProjectService.getProjectIndex).mockRejectedValue(new Error('offline'));

    new ResourceMutationCommittedHandler().handle({ result: emptyResult(2) });

    await vi.waitFor(() => {
      expect(projectPublicationCoordinator.getSnapshotForTests()).toMatchObject({
        phase: 'idle',
        pendingRevisions: [],
        appliedRevision: 0,
      });
    });
    expect(GraphProjectionService.loadGraph).not.toHaveBeenCalled();
    expect(GraphProjectionService.hydrateGraph).not.toHaveBeenCalled();
    expect(invalidateGraphProjection).not.toHaveBeenCalled();
  });
});
