import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ProjectData } from '@/shared/types';
import type { GraphData } from '@/shared/types/store/graph';
import { graphDataToDomainGraph } from '@/shared/types/dto/graphModel';
import { LoadStatus } from '@/shared/types/ui/common';
import { useDatabaseStore } from './databaseStore';
import { useGraphDataStore } from './graphDataStore';
import { useProjectIOStore } from './projectIOStore';
import { useResourceStore, resourceKey, markResourceLoaded } from '@/features/core/resource';
import { useDocumentStateStore } from '@/features/core/resource/documentStateStore';
import { invalidateGraphProjection } from '@/features/application/editorProjection/graphProjectionCoordinator';
import { toGraphResourceUri } from '@/shared/types/domain/graphResourcePath';
import { useVariableStore } from './variableStore';
import { useHistoryStore } from '@/features/core/history';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import type { ResourceMutationResultDto } from '@/shared/types/dto/editorMutation';
import { ProjectService } from '@/services/project/projectService';
import { GraphProjectionService } from '@/services/nodeSystem/graphProjectionService';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';

vi.mock('@/services/project/projectService', () => ({
  ProjectService: {
    getProjectPath: vi.fn(),
    getDatabasesVariables: vi.fn(),
    getProjectIndex: vi.fn(),
  },
}));

vi.mock('@/services/nodeSystem/graphProjectionService', () => ({
  GraphProjectionService: {
    loadGraph: vi.fn(),
    hydrateGraph: vi.fn(),
  },
}));

vi.mock('@/features/application/graphDocument/functionSignatureSync', () => ({
  hydrateFunctionSignaturesFromProjectIndex: vi.fn(),
  syncFunctionSignatureFromGraph: vi.fn(),
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

function makeEventGraphData(path: string, name: string): GraphData {
  return {
    path,
    name,
    type: 'event',
    nodes: [
      {
        id: 'node-a',
        graphPath: path,
        nodeType: 'Control:Begin',
        category: ['Control'],
        title: 'Begin',
        position: { x: 0, y: 0 },
        inputs: [],
        outputs: ['pin-exec'],
      },
    ],
    pins: [
      {
        id: 'pin-exec',
        nodeId: 'node-a',
        name: 'Exec',
        type: 'exec',
        direction: 'output',
      },
    ],
    connections: [],
  };
}

describe('useProjectIOStore snapshot paths', () => {
  beforeEach(() => {
    projectPublicationCoordinator.cancelProject();
    vi.restoreAllMocks();
    vi.clearAllMocks();
    useGraphDataStore.setState({ graphEntities: {} });
    useDatabaseStore.getState().clear();
    useVariableStore.getState().clear();
    useResourceStore.getState().clear();
    useProjectIOStore.setState({
      status: LoadStatus.Idle,
      error: null,
      currentPath: null,
      projectInstanceId: 'project-instance-1',
    });
  });

  it('loadProjectFromData merges database metadata without caching graph bodies', () => {
    useDatabaseStore.getState().setDatabases({
      'df-1': {
        id: 'df-1',
        name: 'Stored Name',
        rowCount: 99,
        columns: [{ name: 'amount', type: 'Float64' }],
      },
    });

    const project: ProjectData = {
      variables: {},
      databases: {
        'df-1': {
          id: 'df-1',
          engine: { csv: { path: '/data/sales.csv' } },
        },
      },
      graphs: {
        'evt-1': graphDataToDomainGraph(makeEventGraphData('evt-1', 'Main Event')),
      },
      metadata: { exportTime: '2026-07-08T00:00:00.000Z', appVersion: '1.0.0' },
    };

    useProjectIOStore.getState().loadProjectFromData(project, '/tmp/demo.yssbi');

    const storedDb = useDatabaseStore.getState().databases['df-1'];
    expect(storedDb.name).toBe('sales');
    expect(storedDb.rowCount).toBe(99);
    expect(storedDb.columns).toEqual([{ name: 'amount', type: 'Float64' }]);
    expect(storedDb.engine).toEqual({ csv: { path: '/data/sales.csv' } });
    expect(useGraphDataStore.getState().hasGraph('evt-1')).toBe(false);
    expect(useProjectIOStore.getState().currentPath).toBeTruthy();
  });

  it('exports graph resource shells without caching imported graph bodies', () => {
    const project: ProjectData = {
      variables: {},
      databases: {},
      graphs: {
        'evt-1': graphDataToDomainGraph(makeEventGraphData('evt-1', 'Main Event')),
      },
      metadata: { exportTime: '2026-07-08T00:00:00.000Z', appVersion: '1.0.0' },
    };

    useProjectIOStore.getState().loadProjectFromData(project, null);
    const snapshot = useProjectIOStore.getState().exportSnapshot();

    expect(snapshot.graphs['evt-1']).toMatchObject({
      path: 'evt-1',
      name: 'Main Event',
      nodes: [],
      pins: [],
      connections: { connections: [] },
    });
  });

  it('loadProject hydrates index and clears graph bodies', async () => {
    vi.mocked(ProjectService.getProjectPath).mockResolvedValue('/tmp/demo.yssbi');
    vi.mocked(ProjectService.getDatabasesVariables).mockResolvedValue({
      databases: { 'df-1': { id: 'df-1', name: 'Data' } },
      variables: {},
    });
    vi.mocked(ProjectService.getProjectIndex).mockResolvedValue({
      projectInstanceId: '00000000-0000-0000-0000-000000000601',
      publicationRevision: 7,
      history: { canUndo: true, canRedo: true },
      projectName: 'Demo',
      graphs: [{ path: 'evt-1', name: 'Main', type: 'event' }],
      variables: [],
      worksheets: [],
      exportTime: '2026-07-08T00:00:00.000Z',
      appVersion: '1.0.0',
    });

    const result = await useProjectIOStore.getState().loadProject();

    expect(result).not.toBeNull();
    expect(useProjectIOStore.getState().status).toBe(LoadStatus.Ready);
    expect(useProjectIOStore.getState().projectInstanceId)
      .toBe('00000000-0000-0000-0000-000000000601');
    expect(useGraphDataStore.getState().hasGraph('evt-1')).toBe(false);
    expect(useResourceStore.getState().graphOrder).toEqual(['evt-1']);
    expect(useDatabaseStore.getState().databases['df-1']?.name).toBe('Data');
    expect(projectPublicationCoordinator.getSnapshotForTests()).toMatchObject({
      projectInstanceId: '00000000-0000-0000-0000-000000000601',
      appliedRevision: 7,
      pendingRevisions: [],
    });
    expect(useHistoryStore.getState()).toEqual({
      canUndo: true,
      canRedo: true,
      pending: false,
    });
  });

  it('cancels old direct and event waiters before replacement-project store reset', async () => {
    const projectInstanceId = '00000000-0000-0000-0000-000000000601';
    const oldResult: ResourceMutationResultDto = {
      projectInstanceId,
      publicationRevision: 2,
      moves: [],
      deltas: [],
      projectionReplacements: [],
      projectionStatus: { status: 'complete', expectedGraphPaths: [] },
      history: { canUndo: true, canRedo: false },
    };
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
    const recoverySnapshot = deferred<Awaited<ReturnType<typeof ProjectService.getProjectIndex>>>();
    vi.mocked(ProjectService.getProjectIndex)
      .mockReturnValueOnce(recoverySnapshot.promise)
      .mockResolvedValueOnce({
        projectInstanceId: '00000000-0000-0000-0000-000000000602',
        publicationRevision: 0,
        history: { canUndo: false, canRedo: false },
        projectName: 'Replacement',
        graphs: [],
        variables: [],
        worksheets: [],
        exportTime: '',
        appVersion: '0.2.7',
      });
    vi.mocked(ProjectService.getProjectPath).mockResolvedValue('/tmp/replacement.yssbi');
    vi.mocked(ProjectService.getDatabasesVariables).mockResolvedValue({
      databases: {},
      variables: {},
    });
    const direct = projectPublicationCoordinator.submit({ result: oldResult });
    const event = projectPublicationCoordinator.submit({ result: structuredClone(oldResult) });
    await vi.waitFor(() => expect(ProjectService.getProjectIndex).toHaveBeenCalledOnce());
    const cancelProject = vi.spyOn(projectPublicationCoordinator, 'cancelProject');
    const clearResources = vi.spyOn(useResourceStore.getState(), 'clear');

    const replacement = useProjectIOStore.getState().loadProject();

    await expect(direct).rejects.toMatchObject({ code: 'stale_project_lifecycle' });
    await expect(event).rejects.toMatchObject({ code: 'stale_project_lifecycle' });
    await expect(replacement).resolves.not.toBeNull();
    expect(cancelProject).toHaveBeenCalled();
    expect(clearResources).toHaveBeenCalled();
    expect(cancelProject.mock.invocationCallOrder[0]).toBeLessThan(
      clearResources.mock.invocationCallOrder[0],
    );
    expect(projectPublicationCoordinator.getSnapshotForTests()).toMatchObject({
      projectInstanceId: '00000000-0000-0000-0000-000000000602',
      appliedRevision: 0,
      pendingRevisions: [],
    });
  });

  it('loadGraph skips backend when graph is already cached', async () => {
    const graphPath = 'events/Main.yssbi-event';
    const fixture = makeEditorProjectionFixture({ graphPath, title: 'Main Event' });
    useGraphDataStore.getState().replaceProjection(graphPath, fixture.projection, 1);
    markResourceLoaded({ id: graphPath, kind: 'event' });
    useResourceStore.getState().setSnapshot({
      resources: [
        {
          id: graphPath,
          kind: 'event',
          name: 'Main Event',
          uri: toGraphResourceUri('event', graphPath),
          exists: true,
          loaded: true,
          hasDirtyDocument: false,
          hasStaleDocument: false,
          hasConflictDocument: false,
        },
      ],
      graphOrder: [graphPath],
    });

    const loaded = await useProjectIOStore.getState().loadGraph(graphPath);

    expect(loaded).toBe(true);
    expect(GraphProjectionService.loadGraph).not.toHaveBeenCalled();
  });

  it('loadGraph installs a projection into an empty graph cache', async () => {
    const graphPath = 'events/New.yssbi-event';
    const fixture = makeEditorProjectionFixture({ graphPath, title: 'Loaded projection' });
    vi.mocked(GraphProjectionService.loadGraph).mockResolvedValue(fixture.projection);

    const loaded = await useProjectIOStore.getState().loadGraph(graphPath);

    expect(loaded).toBe(true);
    expect(GraphProjectionService.loadGraph).toHaveBeenCalledWith(
      graphPath,
      'zh-CN',
      expect.any(Number),
      'project-instance-1',
    );
    expect(useGraphDataStore.getState().graphEntities[graphPath]?.nodes['local-node'].title)
      .toBe('Loaded projection');
  });

  it('loadGraph preserves an existing stale projection when IPC fails', async () => {
    const graphPath = 'events/Stale.yssbi-event';
    const fixture = makeEditorProjectionFixture({ graphPath, title: 'Existing projection' });
    useGraphDataStore.getState().replaceProjection(graphPath, fixture.projection, 1);
    markResourceLoaded({ id: graphPath, kind: 'event' });
    useDocumentStateStore.getState().patchDocument(
      resourceKey({ id: graphPath, kind: 'event' }),
      { stale: true },
    );
    const previousBucket = useGraphDataStore.getState().graphEntities[graphPath];
    vi.mocked(GraphProjectionService.loadGraph).mockRejectedValue(new Error('offline'));

    await expect(useProjectIOStore.getState().loadGraph(graphPath)).resolves.toBe(false);

    expect(useGraphDataStore.getState().graphEntities[graphPath]).toBe(previousBucket);
    expect(useDocumentStateStore.getState().documents[resourceKey({ id: graphPath, kind: 'event' })]?.stale)
      .toBe(true);
  });

  it('loadGraph ignores its response after a newer coordinator refresh wins', async () => {
    const graphPath = 'events/Racing.yssbi-event';
    const current = makeEditorProjectionFixture({ graphPath, sourceRevision: 1, title: 'Current' });
    const older = makeEditorProjectionFixture({ graphPath, sourceRevision: 2, title: 'Older load' });
    const newer = makeEditorProjectionFixture({ graphPath, sourceRevision: 3, title: 'Newer refresh' });
    useGraphDataStore.getState().replaceProjection(graphPath, current.projection, 1);
    markResourceLoaded({ id: graphPath, kind: 'event' });
    useDocumentStateStore.getState().patchDocument(
      resourceKey({ id: graphPath, kind: 'event' }),
      { stale: true },
    );
    const pendingLoad = deferred<typeof older.projection>();
    vi.mocked(GraphProjectionService.loadGraph).mockReturnValue(pendingLoad.promise);
    vi.mocked(GraphProjectionService.hydrateGraph).mockResolvedValue(newer.projection);

    const olderRequest = useProjectIOStore.getState().loadGraph(graphPath);
    await expect(invalidateGraphProjection(graphPath)).resolves.toBe(true);
    pendingLoad.resolve(older.projection);
    await expect(olderRequest).resolves.toBe(false);

    expect(useGraphDataStore.getState().graphEntities[graphPath]).toMatchObject({
      sourceRevision: 3,
      nodes: { 'local-node': { title: 'Newer refresh' } },
    });
  });

  it('does not let an old project request clear the current same-path in-flight load', async () => {
    const graphPath = 'events/Main.yssbi-event';
    const previousProjectProjection = makeEditorProjectionFixture({
      graphPath,
      sourceRevision: 1,
      title: 'Previous project',
    });
    const currentProjectProjection = makeEditorProjectionFixture({
      graphPath,
      sourceRevision: 1,
      title: 'Current project',
    });
    const previousPending = deferred<typeof previousProjectProjection.projection>();
    const currentPending = deferred<typeof currentProjectProjection.projection>();
    vi.mocked(GraphProjectionService.loadGraph)
      .mockReturnValueOnce(previousPending.promise)
      .mockReturnValueOnce(currentPending.promise);

    const previousRequest = useProjectIOStore.getState().loadGraph(graphPath);
    useProjectIOStore.getState().loadProjectFromData({
      variables: {},
      databases: {},
      graphs: {
        [graphPath]: graphDataToDomainGraph(makeEventGraphData(graphPath, 'Main')),
      },
      metadata: { exportTime: '2026-07-25T00:00:00.000Z', appVersion: '1.0.0' },
    }, null);
    useProjectIOStore.setState({ projectInstanceId: 'project-instance-2' });
    const currentRequest = useProjectIOStore.getState().loadGraph(graphPath);

    previousPending.resolve(previousProjectProjection.projection);
    await expect(previousRequest).resolves.toBe(false);
    const deduplicatedRequest = useProjectIOStore.getState().loadGraph(graphPath);

    expect(GraphProjectionService.loadGraph).toHaveBeenCalledTimes(2);
    currentPending.resolve(currentProjectProjection.projection);
    await expect(Promise.all([currentRequest, deduplicatedRequest])).resolves.toEqual([true, true]);
    expect(useGraphDataStore.getState().graphEntities[graphPath]?.nodes['local-node'].title)
      .toBe('Current project');
  });

  it('loadGraph replaces a stale cached projection', async () => {
    const graphPath = 'events/Stale.yssbi-event';
    const fixture = makeEditorProjectionFixture({ graphPath, title: 'Stale Event' });
    useGraphDataStore.getState().replaceProjection(graphPath, fixture.projection, 1);
    markResourceLoaded({ id: graphPath, kind: 'event' });
    useDocumentStateStore.getState().patchDocument(
      resourceKey({ id: graphPath, kind: 'event' }),
      { stale: true },
    );

    const refreshed = makeEditorProjectionFixture({
      graphPath,
      sourceRevision: 2,
      title: 'Refreshed projection',
    });
    vi.mocked(GraphProjectionService.loadGraph).mockResolvedValue(refreshed.projection);

    const loaded = await useProjectIOStore.getState().loadGraph(graphPath);

    expect(loaded).toBe(true);
    expect(GraphProjectionService.loadGraph).toHaveBeenCalledWith(
      graphPath,
      'zh-CN',
      expect.any(Number),
      'project-instance-1',
    );
    expect(useGraphDataStore.getState().graphEntities[graphPath]).toMatchObject({
      sourceRevision: 2,
      nodes: { 'local-node': { title: 'Refreshed projection' } },
    });
    expect(useDocumentStateStore.getState().documents[resourceKey({ id: graphPath, kind: 'event' })]?.stale)
      .toBe(false);
  });
});
