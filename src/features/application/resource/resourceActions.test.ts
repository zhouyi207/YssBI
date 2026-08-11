import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useDatabaseStore, useGraphDataStore, useGraphMetaStore } from '@/features/core/dataStore';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import { startProjectLifecycle } from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import { useResourceStore } from '@/features/core/resource';
import { DatabaseService } from '@/services/database/databaseService';
import { GraphService } from '@/services/graph/graphService';
import { WorksheetService } from '@/services/worksheet/worksheetService';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import { closeEditorTab } from '@/features/application/editor/closeEditorTab';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import { deleteResource, renameResource } from './resourceActions';

vi.mock('@/features/application/editorMutation/projectPublicationCoordinator', () => ({
  projectPublicationCoordinator: {
    submit: vi.fn(async () => ({ status: 'applied', affectedGraphPaths: new Set() })),
    capturePublicationRevision: vi.fn(() => 0),
  },
}));

vi.mock('@/features/application/editor/closeEditorTab', () => ({
  closeEditorTab: vi.fn(async () => true),
}));

function databaseResult(afterName: string | null, operationId: string) {
  const before = {
    id: 'sales',
    engine: { duckDb: { path: 'database/project.duckdb', table: 'sales' } },
    schemaVersion: 1,
    required: false,
    name: 'Sales',
  };
  return {
    data: null,
    mutation: {
      operationId,
      projectInstanceId: 'project-instance-current',
      publicationRevision: 1,
      moves: [],
      deltas: [{
        resource: { kind: 'database' as const, key: 'opaque database resource path' },
        fromRevision: 4,
        toRevision: 5,
        causedBy: operationId,
        payload: {
          kind: 'database' as const,
          patch: { before, after: afterName === null ? null : { ...before, name: afterName } },
        },
      }],
      projectionReplacements: [],
      projectionStatus: { status: 'complete' as const, expectedGraphPaths: [] },
      history: { canUndo: false, canRedo: false },
    },
  };
}

function deleteResult(projectInstanceId: string) {
  return {
    operationId: '00000000-0000-0000-0000-000000000123',
    projectInstanceId,
    publicationRevision: 1,
    moves: [],
    deltas: [{
      resource: { kind: 'graph' as const, key: 'events/Old.yssbi-event' },
      fromRevision: 0,
      toRevision: 1,
      causedBy: '00000000-0000-0000-0000-000000000123',
      payload: {
        kind: 'resource_lifecycle' as const,
        patch: {
          before: {
            path: 'events/Old.yssbi-event',
            kind: 'event' as const,
            name: 'Old',
            revision: 0,
          },
          after: null,
        },
      },
    }],
    projectionReplacements: [],
    projectionStatus: {
      status: 'complete' as const,
      expectedGraphPaths: [],
    },
    history: { canUndo: true, canRedo: false },
  };
}

function worksheetRenameResult(projectInstanceId: string, publicationRevision = 1) {
  return {
    operationId: '00000000-0000-0000-0000-000000000124',
    projectInstanceId,
    publicationRevision,
    moves: [{
      from: 'worksheets/Report.yssbi-worksheet',
      to: 'worksheets/Renamed Report.yssbi-worksheet',
      kind: 'worksheet' as const,
      name: 'Renamed Report',
    }],
    deltas: [{
      resource: { kind: 'worksheet' as const, key: 'worksheets/Renamed Report.yssbi-worksheet' },
      fromRevision: 4,
      toRevision: 5,
      causedBy: '00000000-0000-0000-0000-000000000124',
      payload: {
        kind: 'resource_move' as const,
        patch: {
          from: 'worksheets/Report.yssbi-worksheet',
          to: 'worksheets/Renamed Report.yssbi-worksheet',
        },
      },
    }],
    projectionReplacements: [],
    projectionStatus: {
      status: 'complete' as const,
      expectedGraphPaths: [],
    },
    history: { canUndo: false, canRedo: false },
  };
}

function renameResult(projectInstanceId: string, publicationRevision = 1) {
  return {
    operationId: '00000000-0000-0000-0000-000000000123',
    projectInstanceId,
    publicationRevision,
    moves: [{
      from: 'events/Old.yssbi-event',
      to: 'events/New.yssbi-event',
      kind: 'event' as const,
      name: 'New',
    }],
    deltas: [{
      resource: { kind: 'graph' as const, key: 'events/New.yssbi-event' },
      fromRevision: 0,
      toRevision: 1,
      causedBy: '00000000-0000-0000-0000-000000000123',
      payload: {
        kind: 'resource_move' as const,
        patch: {
          from: 'events/Old.yssbi-event',
          to: 'events/New.yssbi-event',
        },
      },
    }],
    projectionReplacements: [],
    projectionStatus: {
      status: 'incomplete' as const,
      invalidatedGraphPaths: ['events/New.yssbi-event'],
    },
    history: { canUndo: true, canRedo: false },
  };
}

describe('renameResource project ownership', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.clearAllMocks();
    useResourceStore.getState().clear();
    useGraphDataStore.setState({ graphEntities: {} });
    useResourceStore.getState().setSnapshot({
      resources: [
        {
          id: 'events/Old.yssbi-event',
          kind: 'event',
          name: 'Old',
          uri: 'yssbi://event/events/Old.yssbi-event',
          revision: 0,
          exists: true,
          loaded: false,
          hasDirtyDocument: false,
          hasStaleDocument: false,
          hasConflictDocument: false,
        },
        {
          id: 'worksheets/Report.yssbi-worksheet',
          kind: 'worksheet',
          name: 'Report',
          uri: 'yssbi://worksheet/worksheets/Report.yssbi-worksheet',
          revision: 4,
          exists: true,
          loaded: true,
          hasDirtyDocument: false,
          hasStaleDocument: false,
          hasConflictDocument: false,
        },
      ],
      graphOrder: ['events/Old.yssbi-event'],
    });
    useGraphMetaStore.getState().clear();
    useDatabaseStore.setState({
      databases: {
        sales: { id: 'sales', name: 'Sales', resourcePath: 'opaque database resource path' },
      },
      revisions: { sales: 4 },
    });
    useProjectIOStore.setState({
      projectInstanceId: 'project-instance-current',
      refreshResourceIndex: vi.fn(async () => true),
    });
    startProjectLifecycle('project-instance-current');
  });

  it('routes database rename and delete through exact revisioned canonical receipts', async () => {
    let renamed!: ReturnType<typeof databaseResult>;
    let deleted!: ReturnType<typeof databaseResult>;
    vi.spyOn(DatabaseService, 'renameDatabase').mockImplementation(
      async (_project, operation) => (renamed = databaseResult('Renamed', operation)),
    );
    vi.spyOn(DatabaseService, 'deleteDatabase').mockImplementation(
      async (_project, operation) => (deleted = databaseResult(null, operation)),
    );

    await renameResource({ id: 'sales', kind: 'database' }, 'Renamed');
    await deleteResource({ id: 'sales', kind: 'database' });

    expect(DatabaseService.renameDatabase).toHaveBeenCalledWith(
      'project-instance-current',
      expect.any(String),
      4,
      'sales',
      'Renamed',
    );
    expect(DatabaseService.deleteDatabase).toHaveBeenCalledWith(
      'project-instance-current',
      expect.any(String),
      4,
      'sales',
    );
    expect(projectPublicationCoordinator.submit).toHaveBeenNthCalledWith(1, { result: renamed.mutation });
    expect(projectPublicationCoordinator.submit).toHaveBeenNthCalledWith(2, { result: deleted.mutation });
  });

  it('uses the loaded graph projection revision instead of stale sidebar metadata for delete', async () => {
    const committed = deleteResult('project-instance-current');
    vi.spyOn(GraphService, 'removeGraph').mockResolvedValue(committed);
    useGraphDataStore.getState().replaceProjection(
      'events/Old.yssbi-event',
      makeEditorProjectionFixture({
        graphPath: 'events/Old.yssbi-event',
        sourceRevision: 3,
      }).projection,
      1,
    );

    await deleteResource({ id: 'events/Old.yssbi-event', kind: 'event' });

    expect(GraphService.removeGraph).toHaveBeenCalledWith(
      'project-instance-current',
      'events/Old.yssbi-event',
      3,
      expect.any(String),
    );
  });

  it('submits the authoritative delete publication without starting a competing tab unload', async () => {
    const committed = deleteResult('project-instance-current');
    vi.spyOn(GraphService, 'removeGraph').mockResolvedValue(committed);

    await deleteResource({ id: 'events/Old.yssbi-event', kind: 'event' });

    expect(closeEditorTab).not.toHaveBeenCalled();
    expect(GraphService.removeGraph).toHaveBeenCalledWith(
      'project-instance-current',
      'events/Old.yssbi-event',
      0,
      expect.any(String),
    );
    expect(projectPublicationCoordinator.submit).toHaveBeenCalledWith({ result: committed });
  });

  it('renames a worksheet from captured revision and token without load or save fallback', async () => {
    const committed = worksheetRenameResult('project-instance-current');
    vi.spyOn(WorksheetService, 'renameWorksheet').mockImplementation(async () => {
      useResourceStore.getState().patchResource(
        { id: 'worksheets/Report.yssbi-worksheet', kind: 'worksheet' },
        { revision: 99 },
      );
      return committed;
    });
    const load = vi.spyOn(WorksheetService, 'loadWorksheet');
    const save = vi.spyOn(WorksheetService, 'saveWorksheet');

    await renameResource(
      { id: 'worksheets/Report.yssbi-worksheet', kind: 'worksheet' },
      'Renamed Report',
    );

    expect(WorksheetService.renameWorksheet).toHaveBeenCalledWith(
      'project-instance-current',
      expect.any(String),
      'worksheets/Report.yssbi-worksheet',
      4,
      'Renamed Report',
      expect.any(Number),
    );
    expect(load).not.toHaveBeenCalled();
    expect(save).not.toHaveBeenCalled();
    expect(projectPublicationCoordinator.submit).toHaveBeenCalledWith({ result: committed });
  });

  it('rejects stale worksheet project and lifecycle ownership before publication', async () => {
    vi.spyOn(WorksheetService, 'renameWorksheet')
      .mockResolvedValueOnce(worksheetRenameResult('project-instance-stale'));

    await expect(renameResource(
      { id: 'worksheets/Report.yssbi-worksheet', kind: 'worksheet' },
      'Renamed Report',
    )).rejects.toThrow('stale project lifecycle');
    expect(projectPublicationCoordinator.submit).not.toHaveBeenCalled();

    let resolveFirst!: (result: ReturnType<typeof worksheetRenameResult>) => void;
    let resolveSecond!: (result: ReturnType<typeof worksheetRenameResult>) => void;
    vi.mocked(WorksheetService.renameWorksheet)
      .mockReset()
      .mockReturnValueOnce(new Promise((resolve) => { resolveFirst = resolve; }))
      .mockReturnValueOnce(new Promise((resolve) => { resolveSecond = resolve; }));

    const first = renameResource(
      { id: 'worksheets/Report.yssbi-worksheet', kind: 'worksheet' },
      'Renamed Report',
    );
    await vi.waitFor(() => expect(WorksheetService.renameWorksheet).toHaveBeenCalledTimes(1));
    const second = renameResource(
      { id: 'worksheets/Report.yssbi-worksheet', kind: 'worksheet' },
      'Renamed Report',
    );
    await vi.waitFor(() => expect(WorksheetService.renameWorksheet).toHaveBeenCalledTimes(2));
    resolveFirst(worksheetRenameResult('project-instance-current', 1));
    await expect(first).rejects.toMatchObject({ code: 'stale_resource_lifecycle' });
    resolveSecond(worksheetRenameResult('project-instance-current', 2));
    await expect(second).resolves.toBeUndefined();
  });

  it('rejects a stale rename receipt before coordinator submission', async () => {
    vi.spyOn(GraphService, 'renameGraphResource').mockResolvedValue(
      renameResult('project-instance-stale'),
    );

    await expect(
      renameResource({ id: 'events/Old.yssbi-event', kind: 'event' }, 'New'),
    ).rejects.toThrow('stale project lifecycle');

    expect(projectPublicationCoordinator.submit).not.toHaveBeenCalled();
  });

  it('delegates the canonical rename receipt without installing the destination independently', async () => {
    const committed = renameResult('project-instance-current');
    vi.spyOn(GraphService, 'renameGraphResource').mockResolvedValue(committed);
    useResourceStore.getState().setSnapshot({
      resources: [{
        id: 'events/Old.yssbi-event',
        kind: 'event',
        name: 'Old',
        uri: 'yssbi://event/events/Old.yssbi-event',
        revision: 0,
        exists: true,
        loaded: false,
        hasDirtyDocument: false,
        hasStaleDocument: false,
        hasConflictDocument: false,
      }],
      graphOrder: ['events/Old.yssbi-event'],
    });
    useGraphMetaStore.setState({
      graphs: {
        'events/Old.yssbi-event': {
          path: 'events/Old.yssbi-event',
          name: 'Old',
          type: 'event',
        },
      },
    });
    const resourcesBefore = useResourceStore.getState().resources;
    const graphOrderBefore = useResourceStore.getState().graphOrder;
    const graphMetaBefore = useGraphMetaStore.getState().graphs;

    await renameResource({ id: 'events/Old.yssbi-event', kind: 'event' }, 'New');

    expect(GraphService.renameGraphResource).toHaveBeenCalledWith(
      'project-instance-current',
      'events/Old.yssbi-event',
      0,
      'New',
      expect.any(Number),
      expect.any(String),
    );
    expect(projectPublicationCoordinator.submit).toHaveBeenCalledOnce();
    expect(projectPublicationCoordinator.submit).toHaveBeenCalledWith({ result: committed });
    expect(useResourceStore.getState().resources).toBe(resourcesBefore);
    expect(useResourceStore.getState().graphOrder).toBe(graphOrderBefore);
    expect(useGraphMetaStore.getState().graphs).toBe(graphMetaBefore);
  });

  it('rejects a matching receipt when project ownership changes in flight', async () => {
    let resolveRename!: (value: Awaited<ReturnType<typeof GraphService.renameGraphResource>>) => void;
    vi.spyOn(GraphService, 'renameGraphResource').mockReturnValue(new Promise((resolve) => {
      resolveRename = resolve;
    }));

    const pending = renameResource({ id: 'events/Old.yssbi-event', kind: 'event' }, 'New');
    useProjectIOStore.setState({ projectInstanceId: 'project-instance-replacement' });
    startProjectLifecycle('project-instance-replacement');
    resolveRename(renameResult('project-instance-current'));

    await expect(pending).rejects.toMatchObject({ code: 'stale_project_lifecycle' });
    expect(projectPublicationCoordinator.submit).not.toHaveBeenCalled();
  });
});
