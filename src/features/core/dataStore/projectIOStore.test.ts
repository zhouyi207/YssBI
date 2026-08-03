import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ProjectData } from '@/shared/types';
import type { GraphData } from '@/shared/types/store/graph';
import { graphDataToDomainGraph } from '@/shared/types/dto/graphModel';
import { LoadStatus } from '@/shared/types/ui/common';
import { useDatabaseStore } from './databaseStore';
import { useGraphDataStore } from './graphDataStore';
import {
  loadActivatedProject,
  prepareAuthoritativeProjectLoad,
  useProjectIOStore,
  type AuthoritativeProjectLoadPlanDependencies,
} from './projectIOStore';
import { useResourceStore, resourceKey, markResourceLoaded } from '@/features/core/resource';
import { useDocumentStateStore } from '@/features/core/resource/documentStateStore';
import { invalidateGraphProjection } from '@/features/application/editorProjection/graphProjectionCoordinator';
import { toGraphResourceUri } from '@/shared/types/domain/graphResourcePath';
import { useVariableStore } from './variableStore';
import { useHistoryStore } from '@/features/core/history';
import { useGraphMetaStore } from './graphMetaStore';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { useEditorTabStore } from '@/features/core/layout/editorTabStore';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import type { ResourceMutationResultDto } from '@/shared/types/dto/editorMutation';
import { ProjectService } from '@/services/project/projectService';
import { captureProjectIdentity } from '@/features/core/projectLifecycle/projectLifecycleAuthority';
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

vi.mock('@/features/application/graphDocument/functionSignatureSync', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/features/application/graphDocument/functionSignatureSync')>()),
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
    projectPublicationCoordinator.startProject('project-instance-1', 0);
    useProjectIOStore.setState({
      status: LoadStatus.Idle,
      error: null,
      currentPath: null,
      projectInstanceId: 'project-instance-1',
    });
  });

  it('loadProjectFromData merges database metadata without caching graph bodies', () => {
    useDatabaseStore.setState({
      databases: {
        'df-1': {
          id: 'df-1',
          name: 'Stored Name',
          rowCount: 99,
          columns: [{ name: 'amount', type: 'Float64' }],
        },
      },
      revisions: { 'df-1': 9 },
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
    expect(useDatabaseStore.getState().revisions).toEqual({});
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

  it.each([
    'normalizeDatabases',
    'normalizeVariables',
    'prepareFunctionState',
    'prepareResourceState',
    'prepareLayoutState',
    'validateCoordinatorStart',
  ] as const)('%s preparation failure has zero frontend effects', async (stage) => {
    const projectInstanceId = '00000000-0000-0000-0000-000000000602';
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
    useProjectIOStore.setState({ projectInstanceId });
    vi.mocked(ProjectService.getProjectPath).mockResolvedValue('/tmp/replacement.yssbi');
    vi.mocked(ProjectService.getDatabasesVariables).mockResolvedValue({
      databases: { 'db-new': { id: 'db-new', name: 'New database' } },
      variables: {},
    });
    vi.mocked(ProjectService.getProjectIndex).mockResolvedValue({
      projectInstanceId,
      publicationRevision: 8,
      history: { canUndo: true, canRedo: false },
      projectName: 'Replacement',
      graphs: [{
        path: 'functions/New.yssbi-function',
        name: 'New',
        type: 'function',
        functionRevision: 1,
        functionSignature: { parameters: [], return_type: null },
      }],
      variables: [],
      worksheets: [],
      databases: [],
      exportTime: '',
      appVersion: '0.2.7',
    });
    useDatabaseStore.setState({ databases: { old: { id: 'old', name: 'Old' } } });
    useVariableStore.setState({ variables: {}, revisions: {} });
    useGraphMetaStore.setState({ graphs: {} });
    useWorksheetStore.setState({ index: [], documents: {} });
    useResourceStore.setState({ resources: {}, graphOrder: ['old-graph'] });
    useEditorTabStore.setState({ registry: {}, placements: {} });
    const before = {
      projectIO: useProjectIOStore.getState(),
      databases: useDatabaseStore.getState(),
      variables: useVariableStore.getState(),
      functions: useGraphMetaStore.getState(),
      worksheets: useWorksheetStore.getState(),
      resources: useResourceStore.getState(),
      tabs: useEditorTabStore.getState(),
      layout: useLayoutStore.getState(),
      coordinator: projectPublicationCoordinator.getSnapshotForTests(),
    };
    const fault = vi.fn(() => {
      throw new Error(`${stage} failed`);
    });

    await expect(prepareAuthoritativeProjectLoad(
      captureProjectIdentity(),
      { [stage]: fault } as Partial<AuthoritativeProjectLoadPlanDependencies>,
    )).rejects.toThrow(`${stage} failed`);

    expect(useProjectIOStore.getState()).toBe(before.projectIO);
    expect(useDatabaseStore.getState()).toBe(before.databases);
    expect(useVariableStore.getState()).toBe(before.variables);
    expect(useGraphMetaStore.getState()).toBe(before.functions);
    expect(useWorksheetStore.getState()).toBe(before.worksheets);
    expect(useResourceStore.getState()).toBe(before.resources);
    expect(useEditorTabStore.getState()).toBe(before.tabs);
    expect(useLayoutStore.getState()).toBe(before.layout);
    expect(projectPublicationCoordinator.getSnapshotForTests()).toEqual(before.coordinator);
  });

  it('authoritatively replaces variable and database resource path metadata from ProjectIndex', async () => {
    const projectInstanceId = '00000000-0000-0000-0000-000000000601';
    const variableId = '00000000-0000-0000-0000-000000000602';
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
    useProjectIOStore.setState({ projectInstanceId });
    useVariableStore.setState({
      variables: {
        [variableId]: {
          id: variableId,
          resourcePath: 'old-opaque-variable-path',
          name: 'Counter',
          dataType: { kind: 'Int64' },
          dataValue: { kind: 'Int64', value: 1 },
          description: '',
          scope: { type: 'global' },
          tags: [],
        },
      },
      revisions: { [variableId]: 1 },
    });
    useDatabaseStore.setState({
      databases: {
        sales: { id: 'sales', name: 'Sales', resourcePath: 'old-opaque-database-path' },
      },
      revisions: { sales: 1 },
    });
    vi.mocked(ProjectService.getProjectIndex).mockResolvedValue({
      projectInstanceId,
      publicationRevision: 1,
      history: { canUndo: false, canRedo: false },
      projectName: 'Current',
      graphs: [],
      variables: [{
        id: variableId,
        resourcePath: 'new-opaque-variable-path',
        revision: 2,
        name: 'Counter',
        dataType: { kind: 'Int64' },
        dataValue: { kind: 'Int64', value: 2 },
        description: '',
        scope: { type: 'global' },
        tags: [],
      }],
      databases: [{
        id: 'sales',
        resourcePath: 'new-opaque-database-path',
        revision: 7,
        engine: { inMemory: { name: 'sales' } },
        schemaVersion: 1,
        required: false,
        name: 'Sales',
      }],
      worksheets: [],
      exportTime: '',
      appVersion: '0.2.7',
    });

    await expect(useProjectIOStore.getState().refreshResourceIndex()).resolves.toBe(true);

    expect(useVariableStore.getState().variables[variableId]?.resourcePath)
      .toBe('new-opaque-variable-path');
    expect(useDatabaseStore.getState().databases.sales?.resourcePath)
      .toBe('new-opaque-database-path');
    expect(useDatabaseStore.getState().revisions.sales).toBe(7);
  });

  it('rejects an index completion from a replaced project before resetting or hydrating stores', async () => {
    const projectInstanceId = '00000000-0000-0000-0000-000000000601';
    projectPublicationCoordinator.startProject(projectInstanceId, 3);
    useProjectIOStore.setState({ projectInstanceId });
    useVariableStore.setState({
      variables: {
        replacement: { id: 'replacement' } as never,
      },
      revisions: { replacement: 1 },
    });
    const request = deferred<Awaited<ReturnType<typeof ProjectService.getProjectIndex>>>();
    vi.mocked(ProjectService.getProjectIndex).mockReturnValue(request.promise);

    const refresh = useProjectIOStore.getState().refreshResourceIndex();
    await vi.waitFor(() => expect(ProjectService.getProjectIndex).toHaveBeenCalledOnce());
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
    request.resolve({
      projectInstanceId,
      publicationRevision: 3,
      history: { canUndo: false, canRedo: false },
      projectName: 'Stale project',
      graphs: [],
      variables: [],
      worksheets: [],
      databases: [],
      exportTime: '',
      appVersion: '0.2.7',
    });

    await expect(refresh).resolves.toBe(false);
    expect(Object.keys(useVariableStore.getState().variables)).toEqual(['replacement']);
  });

  it('captures one identity epoch for path databases and index hydration', async () => {
    const projectInstanceId = '00000000-0000-0000-0000-000000000601';
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
    useProjectIOStore.setState({ projectInstanceId });
    const pathRequest = deferred<string | null>();
    vi.mocked(ProjectService.getProjectPath).mockReturnValue(pathRequest.promise);
    vi.mocked(ProjectService.getDatabasesVariables).mockResolvedValue({ databases: {}, variables: {} });
    vi.mocked(ProjectService.getProjectIndex).mockResolvedValue({
      projectInstanceId,
      publicationRevision: 0,
      history: { canUndo: false, canRedo: false },
      projectName: 'Stale project',
      graphs: [],
      variables: [],
      worksheets: [],
      databases: [],
      exportTime: '',
      appVersion: '0.2.7',
    });

    const preparation = prepareAuthoritativeProjectLoad(captureProjectIdentity());
    await vi.waitFor(() => expect(ProjectService.getProjectPath).toHaveBeenCalledOnce());
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
    pathRequest.resolve('/tmp/stale.yssbi');

    await expect(preparation).rejects.toMatchObject({ code: 'stale_project_lifecycle' });
    expect(ProjectService.getDatabasesVariables).not.toHaveBeenCalled();
    expect(ProjectService.getProjectIndex).not.toHaveBeenCalled();
  });

  it('establishes a new identity owner before first activation hydration', async () => {
    const projectInstanceId = '00000000-0000-0000-0000-000000000701';
    projectPublicationCoordinator.cancelProject();
    vi.mocked(ProjectService.getProjectPath).mockResolvedValue('/tmp/first.yssbi');
    vi.mocked(ProjectService.getDatabasesVariables).mockResolvedValue({ databases: {}, variables: {} });
    vi.mocked(ProjectService.getProjectIndex).mockResolvedValue({
      projectInstanceId,
      publicationRevision: 2,
      history: { canUndo: false, canRedo: false },
      projectName: 'First',
      graphs: [],
      variables: [],
      worksheets: [],
      databases: [],
      exportTime: '',
      appVersion: '0.2.7',
    });

    const activation = {
      path: '/tmp/first.yssbi',
      projectInstanceId,
      activationRevision: 1001,
    };
    const direct = loadActivatedProject(activation);
    const event = loadActivatedProject(activation);
    await expect(Promise.all([direct, event])).resolves.toEqual([
      expect.any(Object),
      expect.any(Object),
    ]);

    expect(ProjectService.getProjectPath).toHaveBeenCalledTimes(1);
    expect(ProjectService.getProjectIndex).toHaveBeenCalledWith(projectInstanceId);
    expect(projectPublicationCoordinator.getSnapshotForTests()).toMatchObject({
      projectInstanceId,
      appliedRevision: 2,
    });
    expect(useProjectIOStore.getState().projectInstanceId).toBe(projectInstanceId);
  });

  it('does not let an old in-flight hydration absorb a replacement activation', async () => {
    const oldId = '00000000-0000-0000-0000-000000000702';
    const newId = '00000000-0000-0000-0000-000000000703';
    projectPublicationCoordinator.startProject(oldId, 0);
    const oldPath = deferred<string | null>();
    vi.mocked(ProjectService.getProjectPath).mockImplementation((projectInstanceId) => (
      projectInstanceId === oldId ? oldPath.promise : Promise.resolve('/tmp/new.yssbi')
    ));
    vi.mocked(ProjectService.getDatabasesVariables).mockResolvedValue({ databases: {}, variables: {} });
    vi.mocked(ProjectService.getProjectIndex).mockImplementation(async (projectInstanceId) => ({
      projectInstanceId,
      publicationRevision: projectInstanceId === newId ? 5 : 1,
      history: { canUndo: false, canRedo: false },
      projectName: projectInstanceId === newId ? 'New' : 'Old',
      graphs: [],
      variables: [],
      worksheets: [],
      databases: [],
      exportTime: '',
      appVersion: '0.2.7',
    }));

    const oldLoad = useProjectIOStore.getState().loadProject();
    await vi.waitFor(() => expect(ProjectService.getProjectPath).toHaveBeenCalledWith(oldId));
    const newLoad = loadActivatedProject({
      path: '/tmp/new.yssbi',
      projectInstanceId: newId,
      activationRevision: 1002,
    });

    await expect(newLoad).resolves.not.toBeNull();
    const callsAfterNewActivation = vi.mocked(ProjectService.getProjectPath).mock.calls.length;
    await expect(loadActivatedProject({
      path: '/tmp/old.yssbi',
      projectInstanceId: oldId,
      activationRevision: 1001,
    })).resolves.toBeNull();
    expect(ProjectService.getProjectPath).toHaveBeenCalledTimes(callsAfterNewActivation);
    oldPath.resolve('/tmp/old.yssbi');
    await expect(oldLoad).resolves.toBeNull();
    expect(projectPublicationCoordinator.getSnapshotForTests()).toMatchObject({
      projectInstanceId: newId,
      appliedRevision: 5,
    });
    expect(useProjectIOStore.getState()).toMatchObject({
      projectInstanceId: newId,
      currentPath: expect.stringContaining('new.yssbi'),
    });
  });

  it('loadProject hydrates index and clears graph bodies', async () => {
    const projectInstanceId = '00000000-0000-0000-0000-000000000601';
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
    useProjectIOStore.setState({ projectInstanceId });
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
      databases: [],
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
      operationId: '00000000-0000-0000-0000-000000000401',
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
        databases: [],
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
    const startProject = vi.spyOn(projectPublicationCoordinator, 'startProject');
    const assignResources = vi.spyOn(useResourceStore, 'setState');
    const replacementProjectInstanceId = '00000000-0000-0000-0000-000000000602';
    projectPublicationCoordinator.startProject(replacementProjectInstanceId, 0);
    useProjectIOStore.setState({ projectInstanceId: replacementProjectInstanceId });

    const replacement = useProjectIOStore.getState().loadProject();

    await expect(direct).rejects.toMatchObject({ code: 'stale_project_lifecycle' });
    await expect(event).rejects.toMatchObject({ code: 'stale_project_lifecycle' });
    await expect(replacement).resolves.not.toBeNull();
    expect(startProject).toHaveBeenCalled();
    expect(assignResources).toHaveBeenCalled();
    expect(startProject.mock.invocationCallOrder[0]).toBeLessThan(
      assignResources.mock.invocationCallOrder[0],
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
    projectPublicationCoordinator.startProject('project-instance-2', 0);
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
