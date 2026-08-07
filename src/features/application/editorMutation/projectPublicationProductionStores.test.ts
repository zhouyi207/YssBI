import { beforeEach, describe, expect, it, vi } from 'vitest';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import {
  useDatabaseStore,
  useGraphDataStore,
  useGraphMetaStore,
  useVariableStore,
} from '@/features/core/dataStore';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { useEditorTabStore } from '@/features/core/layout/editorTabStore';
import {
  buildGraphResourceMeta,
  markResourceLoaded,
  resourceKey,
  useDocumentStateStore,
  useResourceStore,
} from '@/features/core/resource';
import { useViewportStore } from '@/features/core/viewport';
import { viewportScopeKey } from '@/features/core/viewport/viewportScope';
import { useHistoryStore } from '@/features/core/history';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import type { WorksheetDocument } from '@/shared/types/domain/worksheet';
import type { ResourceMoveDto, ResourceMutationResultDto } from '@/shared/types/dto/editorMutation';
import type { ProjectDatabaseIndexRow, ProjectIndexRow } from '@/services/project/projectService';
import {
  ProjectPublicationCoordinator,
  type ProjectPublicationDependencies,
} from './projectPublicationCoordinator';
import { prepareGraphResourceMove } from './projectPublicationMovePlan';
import {
  commitPreparedProjectRecovery,
  prepareProjectRecoveryCommit,
  validateProjectRecoveryIndex,
} from './projectPublicationRecovery';
import {
  commitPreparedPublication,
  prepareSynchronousPublicationCommit,
} from './resourceMutationResult';

const projectInstanceId = '00000000-0000-0000-0000-000000000901';
const firstPath = 'events/First.yssbi-event';
const secondPath = 'events/Second.yssbi-event';
const destinationPath = 'events/Merged.yssbi-event';
const worksheetId = 'worksheet-1';

function databaseIndexRow(
  overrides: Partial<ProjectDatabaseIndexRow> = {},
): ProjectDatabaseIndexRow {
  return {
    id: 'sales',
    resourcePath: 'opaque-database-resource',
    revision: 4,
    engine: { duckDb: { path: 'database/project.duckdb', table: 'sales' } },
    schemaVersion: 7,
    required: true,
    name: 'Authoritative sales',
    ...overrides,
  };
}

function recoveryIndex(databases: ProjectDatabaseIndexRow[] = []): ProjectIndexRow {
  return {
    projectInstanceId,
    projectName: 'Current',
    appVersion: '0.2.7',
    exportTime: '',
    publicationRevision: 2,
    history: { canUndo: false, canRedo: false },
    graphs: [],
    variables: [],
    worksheets: [],
    databases,
  };
}

function worksheet(name = 'Worksheet'): WorksheetDocument {
  return {
    schemaVersion: 1,
    revision: 1,
    id: worksheetId,
    name,
    databaseId: 'database-1',
    chartType: 'scatter',
    encodings: { x: 'x', y: 'y' },
  };
}

function worksheetResource(document: WorksheetDocument) {
  return {
    id: document.id,
    kind: 'worksheet' as const,
    name: document.name,
    uri: `yssbi://worksheet/${document.id}`,
    exists: true,
    loaded: true,
    hasDirtyDocument: false,
    hasStaleDocument: false,
    hasConflictDocument: false,
  };
}

function move(from: string): ResourceMoveDto {
  return { from, to: destinationPath, kind: 'event', name: 'Merged' };
}

function moveDelta(resourceMove: ResourceMoveDto): ResourceMutationResultDto['deltas'][number] {
  return {
    resource: { kind: 'graph', key: resourceMove.to },
    fromRevision: 0,
    toRevision: 1,
    causedBy: null,
    payload: {
      kind: 'graph_resource_move',
      patch: { from: resourceMove.from, to: resourceMove.to },
    },
  };
}

function snapshotProductionStores() {
  return structuredClone({
    graphEntities: useGraphDataStore.getState().graphEntities,
    graphMeta: useGraphMetaStore.getState().graphs,
    variables: useVariableStore.getState().variables,
    resources: useResourceStore.getState().resources,
    graphOrder: useResourceStore.getState().graphOrder,
    documents: useDocumentStateStore.getState().documents,
    worksheetIndex: useWorksheetStore.getState().index,
    worksheetDocuments: useWorksheetStore.getState().documents,
    databases: useDatabaseStore.getState().databases,
    databaseRevisions: useDatabaseStore.getState().revisions,
    focusedSession: useGraphSessionStore.getState().focusedSession,
    tabs: useEditorTabStore.getState().snapshotMemento(),
    viewports: useViewportStore.getState().viewports,
  });
}

function resetProductionStores(): void {
  useGraphDataStore.setState({ graphEntities: {} });
  useGraphMetaStore.setState({ graphs: {} });
  useVariableStore.setState({ variables: {}, revisions: {} });
  useDatabaseStore.setState({ databases: {}, revisions: {} });
  useResourceStore.getState().clear();
  useDocumentStateStore.getState().clear();
  useWorksheetStore.getState().clear();
  useGraphSessionStore.getState().reset();
  useEditorTabStore.setState({ registry: {}, placements: {} });
  useViewportStore.getState().clear();
  useHistoryStore.setState({ canUndo: false, canRedo: false, pending: false });
}

describe('project publication production stores', () => {
  beforeEach(resetProductionStores);

  it('applies canonical database deltas to the database revision authority projection', () => {
    const operationId = '00000000-0000-0000-0000-000000000904';
    const before = {
      id: 'sales',
      engine: { duckDb: { path: 'database/project.duckdb', table: 'sales' } },
      schemaVersion: 1,
      required: false,
      name: 'Before',
    };
    const after = { ...before, name: 'After' };
    useDatabaseStore.setState({
      databases: {
        sales: {
          ...before,
          name: 'Before',
          columns: [{ name: 'amount', type: 'Float64' }],
          resourcePath: 'opaque database resource path',
        },
      },
      revisions: { sales: 4 },
    });
    useResourceStore.getState().upsertResource({
      id: 'sales',
      kind: 'database',
      name: 'Before',
      uri: 'yssbi://database/sales',
      exists: true,
      loaded: true,
      hasDirtyDocument: false,
      hasStaleDocument: false,
      hasConflictDocument: false,
    });
    const result: ResourceMutationResultDto = {
      operationId,
      projectInstanceId,
      publicationRevision: 1,
      moves: [],
      deltas: [{
        resource: { kind: 'database', key: 'opaque database resource path' },
        fromRevision: 4,
        toRevision: 5,
        causedBy: operationId,
        payload: { kind: 'database', patch: { before, after } },
      }],
      projectionReplacements: [],
      projectionStatus: { status: 'complete', expectedGraphPaths: [] },
      history: { canUndo: false, canRedo: false },
    };

    const plan = prepareSynchronousPublicationCommit(result, {
      projectInstanceId,
      epoch: 1,
      fingerprint: 'database',
      affectedGraphPaths: new Set(),
      moves: [],
    });
    commitPreparedPublication(plan);

    expect(useDatabaseStore.getState().revisions.sales).toBe(5);
    expect(useDatabaseStore.getState().databases.sales).toMatchObject({
      name: 'After',
      columns: [{ name: 'amount', type: 'Float64' }],
      resourcePath: 'opaque database resource path',
    });
    expect(useResourceStore.getState().resources['yssbi://database/sales']?.name).toBe('After');
  });

  it('moves unloaded ownership without installing or loading a graph when complete replacements are empty', () => {
    const source = buildGraphResourceMeta('event', firstPath, 'First');
    useResourceStore.getState().setSnapshot({ resources: [source], graphOrder: [firstPath] });
    useDocumentStateStore.getState().upsertDocument({
      resourceKey: resourceKey(source),
      loaded: false,
      dirty: true,
      stale: false,
      missing: false,
      conflict: false,
      version: 3,
    });
    useGraphMetaStore.getState().addGraph({ path: firstPath, name: 'First', type: 'event' });
    useEditorTabStore.getState().initGroupPlacement('editor', [
      { id: firstPath, component: 'GraphEditor', type: 'event' },
    ], firstPath);
    useViewportStore.getState().setViewport({ groupId: 'editor', graphPath: firstPath }, {
      x: 7,
      y: 9,
      scale: 1.25,
    });
    const resourceMove: ResourceMoveDto = {
      from: firstPath,
      to: destinationPath,
      kind: 'event',
      name: 'Merged',
    };
    const preparedMove = prepareGraphResourceMove(resourceMove, false);
    const result: ResourceMutationResultDto = {
      operationId: '00000000-0000-0000-0000-000000000906',
      projectInstanceId,
      publicationRevision: 1,
      moves: [resourceMove],
      deltas: [moveDelta(resourceMove)],
      projectionReplacements: [],
      projectionStatus: { status: 'complete', expectedGraphPaths: [] },
      history: { canUndo: true, canRedo: false },
    };

    const plan = prepareSynchronousPublicationCommit(result, {
      projectInstanceId,
      epoch: 1,
      fingerprint: 'unloaded-complete-move',
      affectedGraphPaths: new Set([firstPath, destinationPath]),
      moves: [preparedMove],
    });
    commitPreparedPublication(plan);

    expect(useGraphDataStore.getState().graphEntities).toEqual({});
    expect(useResourceStore.getState().resources).toMatchObject({
      [resourceKey({ id: destinationPath, kind: 'event' })]: {
        id: destinationPath,
        loaded: false,
      },
    });
    expect(useDocumentStateStore.getState().documents).toMatchObject({
      [resourceKey({ id: destinationPath, kind: 'event' })]: {
        loaded: false,
        dirty: true,
        version: 3,
      },
    });
    expect(useEditorTabStore.getState().getPlacement('editor')).toMatchObject({
      tabIds: [destinationPath],
      activeTabId: destinationPath,
    });
    expect(useViewportStore.getState().viewports).toEqual({
      [viewportScopeKey({ groupId: 'editor', graphPath: destinationPath })]: {
        x: 7,
        y: 9,
        scale: 1.25,
      },
    });
  });

  it('rejects incomplete publication before committing move ownership or graph authority', () => {
    const source = buildGraphResourceMeta('event', firstPath, 'First');
    useResourceStore.getState().setSnapshot({ resources: [source], graphOrder: [firstPath] });
    useGraphMetaStore.getState().addGraph({ path: firstPath, name: 'First', type: 'event' });
    const resourceMove: ResourceMoveDto = {
      from: firstPath,
      to: destinationPath,
      kind: 'event',
      name: 'Merged',
    };
    const preparedMove = prepareGraphResourceMove(resourceMove, false);
    const before = snapshotProductionStores();
    const result: ResourceMutationResultDto = {
      operationId: '00000000-0000-0000-0000-000000000907',
      projectInstanceId,
      publicationRevision: 1,
      moves: [resourceMove],
      deltas: [moveDelta(resourceMove)],
      projectionReplacements: [],
      projectionStatus: { status: 'incomplete', invalidatedGraphPaths: [destinationPath] },
      history: { canUndo: true, canRedo: false },
    };

    expect(() => prepareSynchronousPublicationCommit(result, {
      projectInstanceId,
      epoch: 1,
      fingerprint: 'incomplete-move-recovery',
      affectedGraphPaths: new Set([firstPath, destinationPath]),
      moves: [preparedMove],
    })).toThrow('incomplete projection status requires recovery');
    expect(snapshotProductionStores()).toEqual(before);
  });

  it('rejects collective move destination conflicts with zero production-store effects', () => {
    useResourceStore.getState().setSnapshot({
      resources: [
        buildGraphResourceMeta('event', firstPath, 'First'),
        buildGraphResourceMeta('event', secondPath, 'Second'),
      ],
      graphOrder: [firstPath, secondPath],
    });
    for (const path of [firstPath, secondPath]) {
      markResourceLoaded({ id: path, kind: 'event' });
      useGraphDataStore.getState().replaceProjection(
        path,
        makeEditorProjectionFixture({ graphPath: path, title: path }).projection,
        1,
      );
      useGraphMetaStore.getState().addGraph({ path, name: path, type: 'event' });
    }
    const before = snapshotProductionStores();
    const destination = makeEditorProjectionFixture({
      graphPath: destinationPath,
      title: 'Merged',
    }).projection;
    const moves = [move(firstPath), move(secondPath)];
    const preparedMoves = moves.map((entry) => prepareGraphResourceMove(entry, true));
    const result: ResourceMutationResultDto = {
      operationId: '00000000-0000-0000-0000-000000000902',
      projectInstanceId,
      publicationRevision: 1,
      moves,
      deltas: [moveDelta(moves[0])],
      projectionReplacements: [{ graphPath: destinationPath, projection: destination }],
      projectionStatus: { status: 'complete', expectedGraphPaths: [destinationPath] },
      history: { canUndo: true, canRedo: false },
    };

    expect(() => prepareSynchronousPublicationCommit(result, {
      projectInstanceId,
      epoch: 1,
      fingerprint: 'conflict',
      affectedGraphPaths: new Set([firstPath, secondPath, destinationPath]),
      moves: preparedMoves,
    })).toThrow(/conflicting move destination/i);
    expect(snapshotProductionStores()).toEqual(before);
  });

  it('composes valid A to X and B to Y moves into one final production-store snapshot', () => {
    const firstDestination = 'events/X.yssbi-event';
    const secondDestination = 'events/Y.yssbi-event';
    useResourceStore.getState().setSnapshot({
      resources: [
        buildGraphResourceMeta('event', firstPath, 'First'),
        buildGraphResourceMeta('event', secondPath, 'Second'),
      ],
      graphOrder: [firstPath, secondPath],
    });
    for (const path of [firstPath, secondPath]) {
      markResourceLoaded({ id: path, kind: 'event' });
      useGraphDataStore.getState().replaceProjection(
        path,
        makeEditorProjectionFixture({ graphPath: path, title: path }).projection,
        1,
      );
      useGraphMetaStore.getState().addGraph({ path, name: path, type: 'event' });
      useViewportStore.getState().setViewport({ groupId: 'editor', graphPath: path }, {
        x: path === firstPath ? 1 : 2,
        y: 3,
        scale: 1,
      });
    }
    useGraphSessionStore.getState().setFocusedSession('editor', firstPath);
    useEditorTabStore.getState().initGroupPlacement('editor', [
      { id: firstPath, component: 'GraphEditor', type: 'event' },
      { id: secondPath, component: 'GraphEditor', type: 'event' },
    ], firstPath);
    const moves: ResourceMoveDto[] = [
      { from: firstPath, to: firstDestination, kind: 'event', name: 'X' },
      { from: secondPath, to: secondDestination, kind: 'event', name: 'Y' },
    ];
    const destinations = new Map([
      [firstDestination, makeEditorProjectionFixture({ graphPath: firstDestination, title: 'X' }).projection],
      [secondDestination, makeEditorProjectionFixture({ graphPath: secondDestination, title: 'Y' }).projection],
    ]);
    const preparedMoves = moves.map((entry) => prepareGraphResourceMove(entry, true));
    const result: ResourceMutationResultDto = {
      operationId: '00000000-0000-0000-0000-000000000903',
      projectInstanceId,
      publicationRevision: 1,
      moves,
      deltas: moves.map(moveDelta),
      projectionReplacements: moves.map((entry) => ({
        graphPath: entry.to,
        projection: destinations.get(entry.to)!,
      })),
      projectionStatus: {
        status: 'complete',
        expectedGraphPaths: [firstDestination, secondDestination],
      },
      history: { canUndo: true, canRedo: false },
    };
    const plan = prepareSynchronousPublicationCommit(result, {
      projectInstanceId,
      epoch: 1,
      fingerprint: 'batch',
      affectedGraphPaths: new Set([firstPath, secondPath, firstDestination, secondDestination]),
      moves: preparedMoves,
    });

    expect(() => commitPreparedPublication(plan)).not.toThrow();

    expect(useResourceStore.getState().graphOrder).toEqual([firstDestination, secondDestination]);
    expect(Object.values(useResourceStore.getState().resources).map((resource) => resource.id).sort())
      .toEqual([firstDestination, secondDestination].sort());
    expect(Object.keys(useGraphDataStore.getState().graphEntities).sort())
      .toEqual([firstDestination, secondDestination].sort());
    expect(Object.keys(useGraphMetaStore.getState().graphs).sort())
      .toEqual([firstDestination, secondDestination].sort());
    expect(useGraphSessionStore.getState().focusedSession).toEqual({
      groupId: 'editor',
      graphPath: firstDestination,
    });
    expect(useEditorTabStore.getState().getPlacement('editor')).toMatchObject({
      tabIds: [firstDestination, secondDestination],
      activeTabId: firstDestination,
    });
    expect(Object.keys(useViewportStore.getState().viewports).sort()).toEqual([
      viewportScopeKey({ groupId: 'editor', graphPath: firstDestination }),
      viewportScopeKey({ groupId: 'editor', graphPath: secondDestination }),
    ].sort());
  });

  it('has zero effects when nested recovery preparation throws and commits only prepared state', () => {
    const graph = buildGraphResourceMeta('event', firstPath, 'First');
    useResourceStore.getState().setSnapshot({ resources: [graph], graphOrder: [firstPath] });
    markResourceLoaded({ id: firstPath, kind: 'event' });
    useGraphDataStore.getState().replaceProjection(
      firstPath,
      makeEditorProjectionFixture({ graphPath: firstPath, title: 'First' }).projection,
      1,
    );
    useGraphMetaStore.getState().addGraph({ path: firstPath, name: 'First', type: 'event' });
    useGraphSessionStore.getState().setFocusedSession('editor', firstPath);
    useEditorTabStore.getState().initGroupPlacement('editor', [
      { id: firstPath, component: 'GraphEditor', type: 'event' },
    ], firstPath);
    useViewportStore.getState().setViewport({ groupId: 'editor', graphPath: firstPath }, {
      x: 1,
      y: 2,
      scale: 1,
    });
    const index = {
      projectInstanceId,
      projectName: 'Current',
      appVersion: '0.2.7',
      exportTime: '',
      publicationRevision: 2,
      history: { canUndo: true, canRedo: true },
      graphs: [],
      variables: [],
      worksheets: [],
      databases: [],
    };
    const preparation = {
      projectInstanceId,
      epoch: 1,
      publicationRevision: 2,
      index,
      projections: new Map(),
      graphPathsLoadedAtStart: new Set([firstPath]),
      pathRemaps: new Map(),
    };
    const beforeFailure = snapshotProductionStores();
    const malformedPreparation = structuredClone(index);
    malformedPreparation.graphs.push({
      path: 'functions/Broken.yssbi-function',
      name: 'Broken',
      type: 'function',
      functionRevision: 1,
      functionSignature: { parameters: null, return_type: null },
    } as never);

    expect(() => prepareProjectRecoveryCommit({
      ...preparation,
      index: malformedPreparation,
    })).toThrow();
    expect(snapshotProductionStores()).toEqual(beforeFailure);

    const prepared = prepareProjectRecoveryCommit(preparation);
    (prepared.index.graphs as unknown[]).push({
      path: 'functions/LateBroken.yssbi-function',
      name: 'LateBroken',
      type: 'function',
      functionRevision: 1,
      functionSignature: { parameters: null, return_type: null },
    });
    prepared.index.history.canUndo = false;

    expect(() => commitPreparedProjectRecovery(prepared)).not.toThrow();
    expect(useResourceStore.getState().resources).toEqual({});
    expect(useDocumentStateStore.getState().documents).toEqual({});
    expect(useGraphDataStore.getState().graphEntities).toEqual({});
    expect(useGraphMetaStore.getState().graphs).toEqual({});
    expect(useGraphSessionStore.getState().focusedSession).toBeNull();
    expect(useEditorTabStore.getState().registry).toEqual({});
    expect(useEditorTabStore.getState().placements).toEqual({});
    expect(useViewportStore.getState().viewports).toEqual({});
    expect(useHistoryStore.getState()).toMatchObject({ canUndo: true, canRedo: true });
  });

  it('materializes an initial-revision database from ProjectIndex into an empty store', () => {
    const row = databaseIndexRow({ revision: 0 });
    const index = recoveryIndex([row]);
    expect(validateProjectRecoveryIndex(index, projectInstanceId)).toBeUndefined();
    const prepared = prepareProjectRecoveryCommit({
      projectInstanceId,
      epoch: 1,
      publicationRevision: 2,
      index,
      projections: new Map(),
      graphPathsLoadedAtStart: new Set(),
      pathRemaps: new Map(),
    });

    commitPreparedProjectRecovery(prepared);

    expect(useDatabaseStore.getState()).toMatchObject({
      databases: {
        sales: {
          id: 'sales',
          resourcePath: 'opaque-database-resource',
          name: 'Authoritative sales',
          engine: row.engine,
          schemaVersion: 7,
          required: true,
        },
      },
      revisions: { sales: 0 },
    });
    expect(useResourceStore.getState().resources['yssbi://database/sales']).toMatchObject({
      id: 'sales',
      kind: 'database',
      name: 'Authoritative sales',
      exists: true,
    });
  });

  it('replaces canonical database fields while preserving only same-id runtime enrichment', () => {
    useDatabaseStore.setState({
      databases: {
        sales: {
          id: 'sales',
          resourcePath: 'stale-resource-path',
          name: 'Stale name',
          engine: { inMemory: { name: 'stale' } },
          schemaVersion: 1,
          required: false,
          columns: [{ name: 'amount', type: 'Float64' }],
          rowCount: 12,
          columnCount: 1,
          loadError: 'runtime load error',
        },
        unrelated: {
          id: 'unrelated',
          name: 'Must be removed',
          engine: { inMemory: { name: 'unrelated' } },
          schemaVersion: 1,
          required: false,
        },
      },
      revisions: { sales: 1, unrelated: 8 },
    });
    useResourceStore.getState().setSnapshot({
      resources: [{
        id: 'sales',
        kind: 'database',
        name: 'Stale name',
        uri: 'yssbi://database/sales',
        exists: true,
        loaded: false,
        hasDirtyDocument: false,
        hasStaleDocument: false,
        hasConflictDocument: false,
      }, {
        id: 'unrelated',
        kind: 'database',
        name: 'Must be removed',
        uri: 'yssbi://database/unrelated',
        exists: true,
        loaded: false,
        hasDirtyDocument: false,
        hasStaleDocument: false,
        hasConflictDocument: false,
      }],
      graphOrder: [],
    });
    const row = databaseIndexRow();

    const prepared = prepareProjectRecoveryCommit({
      projectInstanceId,
      epoch: 1,
      publicationRevision: 2,
      index: recoveryIndex([row]),
      projections: new Map(),
      graphPathsLoadedAtStart: new Set(),
      pathRemaps: new Map(),
    });
    commitPreparedProjectRecovery(prepared);

    expect(useDatabaseStore.getState().databases).toEqual({
      sales: {
        id: 'sales',
        resourcePath: 'opaque-database-resource',
        name: 'Authoritative sales',
        engine: row.engine,
        schemaVersion: 7,
        required: true,
        columns: [{ name: 'amount', type: 'Float64' }],
        rowCount: 12,
        columnCount: 1,
        loadError: 'runtime load error',
      },
    });
    expect(useDatabaseStore.getState().revisions).toEqual({ sales: 4 });
    expect(useResourceStore.getState().resources).toEqual({
      'yssbi://database/sales': expect.objectContaining({
        name: 'Authoritative sales',
        loaded: false,
      }),
    });
  });

  it('removes database declaration revision and resource together when absent from ProjectIndex', () => {
    useDatabaseStore.setState({
      databases: {
        sales: {
          id: 'sales',
          name: 'Sales',
          engine: { inMemory: { name: 'sales' } },
          schemaVersion: 1,
          required: false,
        },
      },
      revisions: { sales: 3 },
    });
    useResourceStore.getState().upsertResource({
      id: 'sales',
      kind: 'database',
      name: 'Sales',
      uri: 'yssbi://database/sales',
      exists: true,
      loaded: false,
      hasDirtyDocument: false,
      hasStaleDocument: false,
      hasConflictDocument: false,
    });

    const prepared = prepareProjectRecoveryCommit({
      projectInstanceId,
      epoch: 1,
      publicationRevision: 2,
      index: recoveryIndex(),
      projections: new Map(),
      graphPathsLoadedAtStart: new Set(),
      pathRemaps: new Map(),
    });
    commitPreparedProjectRecovery(prepared);

    expect(useDatabaseStore.getState().databases).toEqual({});
    expect(useDatabaseStore.getState().revisions).toEqual({});
    expect(useResourceStore.getState().resources).toEqual({});
  });

  it.each([
    ['missing engine', (({ engine: _engine, ...row }) => row)(databaseIndexRow())],
    ['missing schemaVersion', (({ schemaVersion: _schemaVersion, ...row }) => row)(databaseIndexRow())],
    ['missing required', (({ required: _required, ...row }) => row)(databaseIndexRow())],
    ['missing name', (({ name: _name, ...row }) => row)(databaseIndexRow())],
    ['malformed engine', databaseIndexRow({ engine: { duckDb: { path: 'only-path' } } as never })],
    ['empty id', databaseIndexRow({ id: '' })],
    ['negative revision', databaseIndexRow({ revision: -1 })],
    ['fractional revision', databaseIndexRow({ revision: 0.5 })],
    ['unsafe revision', databaseIndexRow({ revision: Number.MAX_SAFE_INTEGER + 1 })],
    ['empty resource path', databaseIndexRow({ resourcePath: '' })],
  ])('rejects %s in a database recovery row', (_label, malformed) => {
    const index = recoveryIndex([malformed as ProjectDatabaseIndexRow]);

    expect(validateProjectRecoveryIndex(index, projectInstanceId))
      .toBe('recovery database metadata is malformed');
  });

  it('rejects duplicate database recovery IDs', () => {
    const index = recoveryIndex([
      databaseIndexRow(),
      databaseIndexRow({ resourcePath: 'another-opaque-path', revision: 5 }),
    ]);

    expect(validateProjectRecoveryIndex(index, projectInstanceId))
      .toBe('recovery database metadata is malformed');
  });

  it('accepts nullable database names from the Rust ProjectIndex wire shape', () => {
    const index = recoveryIndex([databaseIndexRow({ name: null })]);

    expect(validateProjectRecoveryIndex(index, projectInstanceId)).toBeUndefined();
  });

  it('recovers authoritative variable resource path metadata from ProjectIndex', () => {
    const variableId = '00000000-0000-0000-0000-000000000905';
    useVariableStore.setState({
      variables: {
        [variableId]: {
          id: variableId,
          resourcePath: 'stale-opaque-variable-path',
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
    const plan = prepareProjectRecoveryCommit({
      projectInstanceId,
      epoch: 1,
      publicationRevision: 2,
      index: {
        projectInstanceId,
        projectName: 'Current',
        appVersion: '0.2.7',
        exportTime: '',
        publicationRevision: 2,
        history: { canUndo: false, canRedo: false },
        graphs: [],
        variables: [{
          id: variableId,
          resourcePath: 'recovered-opaque-variable-path',
          revision: 2,
          name: 'Counter',
          dataType: { kind: 'Int64' },
          dataValue: { kind: 'Int64', value: 2 },
          description: '',
          scope: { type: 'global' },
          tags: [],
        }],
        worksheets: [],
        databases: [],
      },
      projections: new Map(),
      graphPathsLoadedAtStart: new Set(),
      pathRemaps: new Map(),
    });

    commitPreparedProjectRecovery(plan);

    expect(useVariableStore.getState().variables[variableId]?.resourcePath)
      .toBe('recovered-opaque-variable-path');
  });

  it('authoritatively removes absent resources documents tabs sessions and worksheets during recovery', () => {
    const graph = buildGraphResourceMeta('event', firstPath, 'First');
    const document = worksheet();
    useResourceStore.getState().setSnapshot({
      resources: [graph, worksheetResource(document)],
      graphOrder: [firstPath],
    });
    markResourceLoaded({ id: firstPath, kind: 'event' });
    useDocumentStateStore.getState().upsertDocument({
      resourceKey: resourceKey(graph),
      loaded: true,
      dirty: true,
      stale: false,
      missing: false,
      conflict: false,
      version: 1,
    });
    useWorksheetStore.setState({ index: [{
      id: document.id,
      name: document.name,
      databaseId: document.databaseId,
      chartType: document.chartType,
    }], documents: { [document.id]: document } });
    useDocumentStateStore.getState().upsertDocument({
      resourceKey: resourceKey({ id: document.id, kind: 'worksheet' }),
      loaded: true,
      dirty: false,
      stale: false,
      missing: false,
      conflict: false,
      version: 1,
    });
    useGraphDataStore.getState().replaceProjection(
      firstPath,
      makeEditorProjectionFixture({ graphPath: firstPath, title: 'First' }).projection,
      1,
    );
    useGraphMetaStore.getState().addGraph({ path: firstPath, name: 'First', type: 'event' });
    useGraphSessionStore.getState().setFocusedSession('editor', firstPath);
    useEditorTabStore.getState().initGroupPlacement('editor', [
      { id: firstPath, component: 'GraphEditor', type: 'event' },
      { id: document.id, component: 'WorksheetEditor', type: 'worksheet' },
    ], firstPath);

    const plan = prepareProjectRecoveryCommit({
      projectInstanceId,
      epoch: 1,
      publicationRevision: 2,
      index: {
        projectInstanceId,
        projectName: 'Current',
        appVersion: '0.2.7',
        exportTime: '',
        publicationRevision: 2,
        history: { canUndo: false, canRedo: false },
        graphs: [],
        variables: [],
        worksheets: [],
        databases: [],
      },
      projections: new Map(),
      graphPathsLoadedAtStart: new Set([firstPath]),
      pathRemaps: new Map(),
    });

    expect(() => commitPreparedProjectRecovery(plan)).not.toThrow();
    expect(useResourceStore.getState().resources).toEqual({});
    expect(useDocumentStateStore.getState().documents).toEqual({});
    expect(useGraphDataStore.getState().graphEntities).toEqual({});
    expect(useGraphMetaStore.getState().graphs).toEqual({});
    expect(useWorksheetStore.getState()).toMatchObject({ index: [], documents: {} });
    expect(useGraphSessionStore.getState().focusedSession).toBeNull();
    expect(useEditorTabStore.getState().registry).toEqual({});
    expect(useEditorTabStore.getState().placements).toEqual({});
  });

  it('applies a matching event and direct worksheet delta exactly once inside publication', async () => {
    const document = worksheet('Created');
    const result = {
      operationId: '00000000-0000-0000-0000-000000000904',
      projectInstanceId,
      publicationRevision: 1,
      moves: [],
      deltas: [],
      worksheetDeltas: [{ id: worksheetId, before: null, after: document }],
      projectionReplacements: [],
      projectionStatus: { status: 'complete', expectedGraphPaths: [] },
      history: { canUndo: true, canRedo: false },
    } satisfies ResourceMutationResultDto;
    const dependencies: ProjectPublicationDependencies = {
      loadRecoverySnapshot: vi.fn(async () => { throw new Error('unexpected recovery'); }),
      prepareGraphProjection: vi.fn(async () => false as const),
      captureLoadedGraphPaths: vi.fn(() => new Set<string>()),
      preparePublication: prepareSynchronousPublicationCommit,
      prepareRecovery: vi.fn((plan) => prepareProjectRecoveryCommit(plan)),
      prepareMove: vi.fn(() => { throw new Error('unexpected move'); }),
      commitPublication: commitPreparedPublication,
      commitRecovery: vi.fn(),
      markProjectProjectionStale: vi.fn(),
    };
    const coordinator = new ProjectPublicationCoordinator(dependencies);
    coordinator.startProject(projectInstanceId, 0);
    let worksheetStoreCommits = 0;
    const unsubscribe = useWorksheetStore.subscribe(() => { worksheetStoreCommits += 1; });

    const event = coordinator.submit({ result });
    const direct = coordinator.submit({ result: structuredClone(result) });

    await expect(event).resolves.toMatchObject({ status: 'applied' });
    await expect(direct).resolves.toMatchObject({ status: 'duplicate' });
    unsubscribe();
    expect(useWorksheetStore.getState().documents).toEqual({ [worksheetId]: document });
    expect(useWorksheetStore.getState().index).toEqual([{
      id: worksheetId,
      name: 'Created',
      databaseId: 'database-1',
      chartType: 'scatter',
    }]);
    expect(worksheetStoreCommits).toBe(1);
  });
});
