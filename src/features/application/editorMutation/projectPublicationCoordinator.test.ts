import { describe, expect, it, vi } from 'vitest';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import { useGraphDataStore, useGraphMetaStore } from '@/features/core/dataStore';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { useEditorTabStore } from '@/features/core/layout/editorTabStore';
import { useEditorStore } from '@/features/core/editor/stores/useEditorStore';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import {
  buildGraphResourceMeta,
  markResourceLoaded,
  resourceKey,
  useDocumentStateStore,
  useResourceStore,
} from '@/features/core/resource';
import { useViewportStore } from '@/features/core/viewport';
import { viewportScopeKey } from '@/features/core/viewport/viewportScope';
import type { EditorGraphProjectionDto } from '@/shared/types/dto/editorProjection';
import type { ResourceMoveDto, ResourceMutationResultDto } from '@/shared/types/dto/editorMutation';
import type { ProjectIndexRow } from '@/services/project/projectService';
import {
  clearWorksheetPreviewCache,
  getCachedWorksheetPreview,
  getWorksheetPreview,
} from '@/services/worksheet/worksheetPreviewCache';
import type { WorksheetDocument } from '@/shared/types/domain';
import {
  ProjectPublicationCoordinator,
  ProjectPublicationError,
  type PreparedProjectPublication,
  type PreparedProjectRecovery,
  type PreparePublicationContext,
  type ProjectPublicationDependencies,
} from './projectPublicationCoordinator';
import type { PreparedGraphResourceMove } from './projectPublicationMovePlan';
import {
  commitPreparedProjectRecovery,
  prepareProjectRecoveryCommit,
} from './projectPublicationRecovery';

const projectInstanceId = '00000000-0000-0000-0000-000000000801';
const replacementProjectInstanceId = '00000000-0000-0000-0000-000000000802';
const beforePath = 'events/Before.yssbi-event';
const afterPath = 'events/After.yssbi-event';
const worksheet: WorksheetDocument = {
  schemaVersion: 3,
  revision: 0,
  databaseId: 'sales',
  chartType: 'scatter',
  encodings: { x: 'x', y: 'y' },
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function projection(graphPath: string): EditorGraphProjectionDto {
  return makeEditorProjectionFixture({ graphPath, title: graphPath }).projection;
}

function publication(
  publicationRevision: number,
  options: {
    moves?: ResourceMoveDto[];
    expectedGraphPaths?: string[];
    invalidatedGraphPaths?: string[];
    functionPath?: string;
    functionRevision?: number;
    history?: { canUndo: boolean; canRedo: boolean };
    operationId?: string;
  } = {},
): ResourceMutationResultDto {
  const projectionPaths = options.expectedGraphPaths;
  const moves = options.moves ?? [];
  const deltas: ResourceMutationResultDto['deltas'] = moves.map((move) => ({
    resource: { kind: move.kind === 'event' ? 'graph' : 'function', key: move.to },
    fromRevision: 0,
    toRevision: 1,
    causedBy: null,
    payload: {
      kind: 'resource_move',
      patch: { from: move.from, to: move.to },
    },
  }));
  if (options.functionPath) {
    deltas.push({
      resource: { kind: 'function', key: options.functionPath },
      fromRevision: (options.functionRevision ?? 1) - 1,
      toRevision: options.functionRevision ?? 1,
      causedBy: null,
      payload: {
        kind: 'function',
        patch: {
          before: { parameters: [], return_type: null },
          after: { parameters: [], return_type: 'Int64' },
        },
      },
    });
  }
  return {
    operationId: options.operationId ?? `00000000-0000-0000-0000-${publicationRevision.toString().padStart(12, '0')}`,
    projectInstanceId,
    publicationRevision,
    moves,
    deltas,
    projectionReplacements: (projectionPaths ?? []).map((graphPath) => ({
      graphPath,
      projection: projection(graphPath),
    })),
    projectionStatus: projectionPaths
      ? { status: 'complete', expectedGraphPaths: projectionPaths }
      : { status: 'incomplete', invalidatedGraphPaths: options.invalidatedGraphPaths ?? [] },
    history: options.history ?? { canUndo: true, canRedo: false },
  };
}

function worksheetCreatePublication(
  publicationRevision: number,
  worksheetPath: string,
): ResourceMutationResultDto {
  return {
    operationId: `00000000-0000-0000-0000-${publicationRevision.toString().padStart(12, '0')}`,
    projectInstanceId,
    publicationRevision,
    moves: [],
    deltas: [{
      resource: { kind: 'worksheet', key: worksheetPath },
      fromRevision: 0,
      toRevision: 0,
      causedBy: `00000000-0000-0000-0000-${publicationRevision.toString().padStart(12, '0')}`,
      payload: {
        kind: 'resource_lifecycle',
        patch: {
          before: null,
          after: {
            revision: 0,
            path: worksheetPath,
            kind: 'worksheet',
            name: 'Recovered worksheet',
          },
        },
      },
    }],
    projectionReplacements: [],
    projectionStatus: { status: 'complete', expectedGraphPaths: [] },
    history: { canUndo: true, canRedo: false },
  };
}

function worksheetMovePublication(
  publicationRevision: number,
  from: string,
  to: string,
  fromRevision: number,
): ResourceMutationResultDto {
  const operationId = `00000000-0000-0000-0000-${publicationRevision.toString().padStart(12, '0')}`;
  return {
    operationId,
    projectInstanceId,
    publicationRevision,
    moves: [{ from, to, kind: 'worksheet', name: `Worksheet ${to}` }],
    deltas: [{
      resource: { kind: 'worksheet', key: to },
      fromRevision,
      toRevision: fromRevision + 1,
      causedBy: operationId,
      payload: { kind: 'resource_move', patch: { from, to } },
    }],
    projectionReplacements: [],
    projectionStatus: { status: 'complete', expectedGraphPaths: [] },
    history: { canUndo: true, canRedo: false },
  };
}

function index(
  publicationRevision: number,
  graphPaths: string[] = [],
  options: Partial<ProjectIndexRow> = {},
): ProjectIndexRow {
  return {
    projectInstanceId,
    projectName: 'Current',

    exportTime: '',
    publicationRevision,
    history: { canUndo: false, canRedo: true },
    graphs: graphPaths.map((path) => ({
      path,
      name: path === afterPath ? 'After' : 'Before',
      type: 'event' as const,
      revision: 0,
    })),
    variables: [],
    worksheets: [],
    ...options,
    databases: options.databases ?? [],
  };
}

interface RecordedProjectionState {
  resources: string[];
  names: Record<string, string>;
  documentFlags: Record<string, { dirty: boolean; stale: boolean; conflict: boolean }>;
  projections: string[];
  functionRevisions: Record<string, number>;
  history: { canUndo: boolean; canRedo: boolean };
  watermark: number;
  commitOrder: number[];
}

function prepareRecordedMove(
  state: RecordedProjectionState,
  move: ResourceMoveDto,
  hasAuthoritativeDestinationReplacement: boolean,
): PreparedGraphResourceMove {
  const flags = state.documentFlags[move.from];
  if (!state.resources.includes(move.from) || !flags) throw new Error('missing move source');
  return {
    from: move.from,
    to: move.to,
    kind: move.kind,
    name: move.name,
    hasAuthoritativeDestinationReplacement,
    resourceSnapshot: { sourceFlags: { ...flags } },
    documentSnapshot: { sourceFlags: { ...flags } },
    tabSnapshot: {},
    sessionSnapshot: {},
  } as unknown as PreparedGraphResourceMove;
}

function commitRecordedPublication(
  state: RecordedProjectionState,
  plan: PreparedProjectPublication,
): void {
  for (const move of plan.moves) {
    const index = state.resources.indexOf(move.from);
    if (index >= 0) state.resources[index] = move.to;
    delete state.names[move.from];
    state.names[move.to] = move.name;
    const flags = state.documentFlags[move.from];
    delete state.documentFlags[move.from];
    state.documentFlags[move.to] = flags;
    state.projections = state.projections.filter((path) => path !== move.from);
  }
  for (const replacement of plan.projectionReplacements) {
    if (!state.projections.includes(replacement.graphPath)) state.projections.push(replacement.graphPath);
  }
  for (const install of plan.functionInstalls) {
    state.functionRevisions[install.graphPath] = install.revision;
  }
  state.history = { ...plan.history };
  state.watermark = plan.publicationRevision;
  state.commitOrder.push(plan.publicationRevision);
}

function commitRecordedRecovery(
  state: RecordedProjectionState,
  plan: PreparedProjectRecovery,
): void {
  state.resources = plan.index.graphs.map((graph) => graph.path);
  state.names = Object.fromEntries(plan.index.graphs.map((graph) => [graph.path, graph.name]));
  state.projections = [...plan.projections.keys()];
  const previousFlags = state.documentFlags;
  const remappedFlags = new Map(
    [...plan.pathRemaps].map(([from, to]) => [to, previousFlags[from]]),
  );
  state.documentFlags = Object.fromEntries(
    state.resources.map((path) => [path, previousFlags[path] ?? remappedFlags.get(path) ?? {
      dirty: false,
      stale: false,
      conflict: false,
    }]),
  );
  state.functionRevisions = Object.fromEntries(
    plan.index.graphs
      .filter((graph) => graph.type === 'function')
      .map((graph) => [graph.path, graph.functionRevision]),
  );
  state.history = { ...plan.index.history };
  state.watermark = plan.publicationRevision;
}

function createHarness() {
  const state: RecordedProjectionState = {
    resources: [beforePath],
    names: { [beforePath]: 'Before' },
    documentFlags: {
      [beforePath]: { dirty: true, stale: false, conflict: false },
    },
    projections: [beforePath],
    functionRevisions: {},
    history: { canUndo: false, canRedo: false },
    watermark: 0,
    commitOrder: [],
  };
  const snapshotRequests: Array<ReturnType<typeof deferred<ProjectIndexRow>>> = [];
  const projectionRequests = new Map<
    string,
    ReturnType<typeof deferred<EditorGraphProjectionDto | false>>
  >();
  const dependencies: ProjectPublicationDependencies = {
    loadRecoverySnapshot: vi.fn(() => {
      const request = deferred<ProjectIndexRow>();
      snapshotRequests.push(request);
      return request.promise;
    }),
    prepareGraphProjection: vi.fn((path) => {
      const request = projectionRequests.get(path);
      if (!request) throw new Error(`missing projection request for ${path}`);
      return request.promise;
    }),
    captureLoadedGraphPaths: vi.fn(() => new Set(state.projections)),
    preparePublication: vi.fn((
      result: ResourceMutationResultDto,
      context: PreparePublicationContext,
    ) => {
      for (const replacement of result.projectionReplacements) {
        if (replacement.projection.nodes.some((node) => node.graphPath !== replacement.graphPath)) {
          throw new Error('nested projection identity is malformed');
        }
      }
      return {
      projectInstanceId: context.projectInstanceId,
      epoch: context.epoch,
      publicationRevision: result.publicationRevision,
      fingerprint: context.fingerprint,
      affectedGraphPaths: context.affectedGraphPaths,
      moves: context.moves,
      removedWorksheetPaths: new Set<string>(),
      graphProjectionPlan: { graphPaths: [], graphEntities: {} },
      projectionReplacements: result.projectionReplacements,
      functionInstalls: result.deltas.flatMap((delta) =>
        delta.resource.kind === 'function' && delta.payload.kind === 'function'
          ? [{
              graphPath: delta.resource.key,
              revision: delta.toRevision,
              signature: delta.payload.patch.after,
              functionInputs: [],
              functionOutputs: [],
            }]
          : []),
      variableInstalls: [],
      storeState: {
        resources: {}, graphOrder: [], documents: {}, graphMeta: {}, databases: {},
        databaseRevisions: {}, variables: {}, variableRevisions: {}, worksheetIndex: [],
        worksheetDocuments: {},
        tabs: { registry: {}, placements: {} }, focusedSession: null, viewports: {},
      },
      history: result.history,
    };
    }),
    prepareRecovery: vi.fn((plan) => ({
      ...plan,
      graphProjectionPlan: { graphPaths: [], graphEntities: {} },
      storeState: {
        resources: {},
        graphOrder: [],
        documents: {},
        graphMeta: {},
        variables: {},
        variableRevisions: {},
        worksheetIndex: [],
        worksheetDocuments: {},
        tabs: { registry: {}, placements: {} },
        focusedSession: null,
        viewports: {},
      },
      history: plan.index.history,
    })),
    prepareMove: vi.fn((move, hasAuthoritativeDestinationReplacement) =>
      prepareRecordedMove(state, move, hasAuthoritativeDestinationReplacement)),
    commitPublication: vi.fn((plan) => commitRecordedPublication(state, plan)),
    commitRecovery: vi.fn((plan) => commitRecordedRecovery(state, plan)),
    markProjectProjectionStale: vi.fn(),
  };
  const coordinator = new ProjectPublicationCoordinator(dependencies);
  coordinator.startProject(projectInstanceId, 0);
  return { coordinator, state, dependencies, snapshotRequests, projectionRequests };
}

function requestProjection(
  harness: ReturnType<typeof createHarness>,
  path: string,
): ReturnType<typeof deferred<EditorGraphProjectionDto | false>> {
  const request = deferred<EditorGraphProjectionDto | false>();
  harness.projectionRequests.set(path, request);
  return request;
}

async function waitForSnapshot(harness: ReturnType<typeof createHarness>, count = 1): Promise<void> {
  await vi.waitFor(() => expect(harness.snapshotRequests).toHaveLength(count));
}

function installReplacementStoreBaseline(graphPath: string): {
  resources: ReturnType<typeof structuredClone>;
  documents: ReturnType<typeof structuredClone>;
} {
  useResourceStore.getState().clear();
  useDocumentStateStore.getState().clear();
  const resource = buildGraphResourceMeta('event', graphPath, 'Replacement');
  useResourceStore.getState().setSnapshot({ resources: [resource], graphOrder: [graphPath] });
  markResourceLoaded({ id: graphPath, kind: 'event' });
  useDocumentStateStore.getState().upsertDocument({
    resourceKey: resourceKey(resource),
    loaded: true,
    dirty: false,
    stale: false,
    missing: false,
    conflict: false,
    version: 1,
  });
  return {
    resources: structuredClone(useResourceStore.getState().resources),
    documents: structuredClone(useDocumentStateStore.getState().documents),
  };
}

function installRealStaleMarker(harness: ReturnType<typeof createHarness>): void {
  vi.mocked(harness.dependencies.markProjectProjectionStale).mockImplementation(() => {
    useResourceStore.setState((state) => ({
      resources: Object.fromEntries(Object.entries(state.resources).map(([key, resource]) => [
        key,
        resource.kind === 'event' || resource.kind === 'function'
          ? { ...resource, hasStaleDocument: true }
          : resource,
      ])),
    }));
    useDocumentStateStore.setState((state) => ({
      documents: Object.fromEntries(Object.entries(state.documents).map(([key, document]) => [
        key,
        { ...document, stale: true },
      ])),
    }));
  });
}

async function flushRejectedRecovery(): Promise<void> {
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
}

const moveAfter: ResourceMoveDto = {
  from: beforePath,
  to: afterPath,
  kind: 'event',
  name: 'After',
};

const moveBefore: ResourceMoveDto = {
  from: afterPath,
  to: beforePath,
  kind: 'event',
  name: 'Before',
};

describe('ProjectPublicationCoordinator', () => {
  it('synchronously clears worksheet previews on every project lifecycle reset', async () => {
    clearWorksheetPreviewCache();
    const harness = createHarness();
    await getWorksheetPreview(
      projectInstanceId,
      'worksheets/Worksheet.yssbi-worksheet',
      worksheet,
      async () => ({ kind: 'empty' }),
    );

    harness.coordinator.startProject(replacementProjectInstanceId, 0);

    expect(getCachedWorksheetPreview(
      projectInstanceId,
      'worksheets/Worksheet.yssbi-worksheet',
      worksheet,
    )).toBeUndefined();
  });

  it('validates a new project baseline before cancelling the current coordinator', () => {
    const harness = createHarness();
    const before = harness.coordinator.getSnapshotForTests();

    expect(() => harness.coordinator.startProject('', -1)).toThrow(
      'project publication baseline is malformed',
    );

    expect(harness.coordinator.getSnapshotForTests()).toEqual(before);
  });

  it('rejects a result without required top-level operation correlation', async () => {
    const harness = createHarness();
    const missing = publication(1) as Partial<ResourceMutationResultDto>;
    delete missing.operationId;

    await expect(harness.coordinator.submit({ result: missing as ResourceMutationResultDto }))
      .rejects.toMatchObject({ code: 'publication_protocol_error' });
  });

  it('queues reverse arrival N+1 then N without installing N+1 first', async () => {
    const harness = createHarness();
    const before = structuredClone(harness.state);
    const revision2 = harness.coordinator.submit({
      result: publication(2, { expectedGraphPaths: [] }),
    });
    await waitForSnapshot(harness);
    const revision1 = harness.coordinator.submit({
      result: publication(1, { expectedGraphPaths: [] }),
    });

    expect(harness.state).toEqual(before);
    harness.snapshotRequests[0].resolve(index(0));
    await expect(revision1).resolves.toMatchObject({ status: 'applied' });
    await expect(revision2).resolves.toMatchObject({ status: 'applied' });
    expect(harness.state.commitOrder).toEqual([1, 2]);
  });

  it('keeps normal apply serialized behind recovery', async () => {
    const harness = createHarness();
    const revision2 = harness.coordinator.submit({
      result: publication(2, { expectedGraphPaths: [] }),
    });
    await waitForSnapshot(harness);

    const revision1 = harness.coordinator.submit({
      result: publication(1, { expectedGraphPaths: [] }),
    });

    await Promise.resolve();
    expect(harness.dependencies.prepareGraphProjection).not.toHaveBeenCalled();

    harness.snapshotRequests[0].resolve(index(0));

    await expect(revision1).resolves.toMatchObject({ status: 'applied' });
    await expect(revision2).resolves.toMatchObject({ status: 'applied' });
  });

  it('recovers an unloaded worksheet create without graph hydration or graph notifications', async () => {
    const harness = createHarness();
    harness.state.projections = [];
    const worksheetPath = 'opaque worksheet recovery::created';
    const submitted = harness.coordinator.submit({
      result: worksheetCreatePublication(2, worksheetPath),
    });
    await waitForSnapshot(harness);

    harness.snapshotRequests[0].resolve(index(2, [], {
      worksheets: [{
        worksheetPath,
        name: 'Recovered worksheet',
        databaseId: 'sales',
        chartType: 'scatter',
        revision: 0,
      }],
    }));

    await expect(submitted).resolves.toEqual({
      status: 'recovered',
      affectedGraphPaths: new Set(),
    });
    expect(harness.dependencies.prepareGraphProjection).not.toHaveBeenCalled();
    expect(harness.dependencies.prepareMove).not.toHaveBeenCalled();
    expect(harness.dependencies.commitPublication).not.toHaveBeenCalled();
    const recovery = vi.mocked(harness.dependencies.prepareRecovery).mock.calls[0][0];
    expect(recovery.projections.size).toBe(0);
    expect(recovery.graphPathsLoadedAtStart.size).toBe(0);
    expect([...(recovery.worksheetPathRemaps ?? [])]).toEqual([]);
  });

  it('recovers an A to B to C worksheet chain without graph work or notifications', async () => {
    const harness = createHarness();
    harness.state.projections = [];
    vi.mocked(harness.dependencies.prepareRecovery).mockImplementation(prepareProjectRecoveryCommit);
    vi.mocked(harness.dependencies.commitRecovery).mockImplementation(commitPreparedProjectRecovery);
    const [pathA, pathB, pathC] = [
      'opaque coordinator worksheet::A',
      'opaque coordinator worksheet::B',
      'opaque coordinator worksheet::C',
    ];
    const documentA = { ...worksheet, revision: 1, encodings: { x: 'old-x', y: 'old-y' } };
    const documentB = { ...worksheet, revision: 2, encodings: { x: 'new-x', y: 'new-y' } };
    useResourceStore.getState().clear();
    useDocumentStateStore.getState().clear();
    useWorksheetStore.setState({
      index: [
        { worksheetPath: pathA, name: 'A', databaseId: 'sales', chartType: 'scatter', revision: 1 },
        { worksheetPath: pathB, name: 'B', databaseId: 'sales', chartType: 'scatter', revision: 2 },
      ],
      documents: { [pathA]: documentA, [pathB]: documentB },
    });
    for (const [path, name, dirty, stale, conflict, version] of [
      [pathA, 'A', false, true, false, 3],
      [pathB, 'B', true, false, true, 8],
    ] as const) {
      const key = resourceKey({ id: path, kind: 'worksheet' });
      useResourceStore.getState().upsertResource({
        id: path,
        kind: 'worksheet',
        name,
        uri: key,
        exists: true,
        loaded: true,
        hasDirtyDocument: dirty,
        hasStaleDocument: stale,
        hasConflictDocument: conflict,
      });
      useDocumentStateStore.getState().upsertDocument({
        resourceKey: key,
        loaded: true,
        dirty,
        stale,
        missing: false,
        conflict,
        version,
      });
    }
    useEditorTabStore.setState({ registry: {}, placements: {} });
    useEditorTabStore.getState().initGroupPlacement('editor', [
      { id: pathA, component: 'WorksheetEditor', type: 'worksheet' },
      { id: pathB, component: 'WorksheetEditor', type: 'worksheet' },
    ], pathB);
    useEditorTabStore.getState().setSelectedTabIds('editor', [pathA, pathB]);
    useEditorStore.getState().setDetailFocus({ kind: 'worksheet', worksheetPath: pathA });

    const first = harness.coordinator.submit({
      result: worksheetMovePublication(2, pathA, pathB, 0),
    });
    await waitForSnapshot(harness);
    const second = harness.coordinator.submit({
      result: worksheetMovePublication(3, pathB, pathC, 1),
    });
    harness.snapshotRequests[0].resolve(index(3, [], {
      worksheets: [{
        worksheetPath: pathC,
        name: 'C',
        databaseId: 'sales',
        chartType: 'scatter',
        revision: 3,
      }],
    }));

    await expect(first).resolves.toEqual({ status: 'recovered', affectedGraphPaths: new Set() });
    await expect(second).resolves.toEqual({ status: 'recovered', affectedGraphPaths: new Set() });
    expect(harness.dependencies.prepareGraphProjection).not.toHaveBeenCalled();
    expect(harness.dependencies.markProjectProjectionStale).not.toHaveBeenCalled();
    expect(useWorksheetStore.getState()).toMatchObject({
      index: [{ worksheetPath: pathC, name: 'C' }],
      documents: { [pathC]: documentB },
    });
    expect(useDocumentStateStore.getState().documents[
      resourceKey({ id: pathC, kind: 'worksheet' })
    ]).toMatchObject({ dirty: true, stale: false, conflict: true, version: 8 });
    expect(useResourceStore.getState().resources[
      resourceKey({ id: pathC, kind: 'worksheet' })
    ]).toMatchObject({
      id: pathC,
      name: 'C',
      hasDirtyDocument: true,
      hasStaleDocument: false,
      hasConflictDocument: true,
    });
    expect(useEditorTabStore.getState().getPlacement('editor')).toMatchObject({
      tabIds: [pathC],
      selectedTabIds: [pathC],
      activeTabId: pathC,
    });
    expect(useEditorStore.getState().detailFocus).toEqual({
      kind: 'worksheet',
      worksheetPath: pathC,
    });
  });

  it('settles late recovery joiners from one stable snapshot attempt', async () => {
    const harness = createHarness();
    const recoveredPath = beforePath;
    const recoveryProjection = requestProjection(harness, recoveredPath);
    const revision2 = harness.coordinator.submit({ result: publication(2) });
    await waitForSnapshot(harness);

    harness.snapshotRequests[0].resolve(index(3, [recoveredPath]));
    await vi.waitFor(() => {
      expect(harness.dependencies.prepareGraphProjection).toHaveBeenCalledWith(
        recoveredPath,
        projectInstanceId,
        expect.any(Number),
      );
    });
    const revision3 = harness.coordinator.submit({ result: publication(3) });
    recoveryProjection.resolve(projection(recoveredPath));

    await expect(revision2).resolves.toMatchObject({ status: 'recovered' });
    await expect(revision3).resolves.toMatchObject({ status: 'recovered' });
    expect(harness.snapshotRequests).toHaveLength(1);
    expect(harness.coordinator.getSnapshotForTests().pendingRevisions).toEqual([]);
  });

  it('applies N+1 immediately after snapshot N recovery commits', async () => {
    const harness = createHarness();
    const recoveryProjection = requestProjection(harness, beforePath);
    const revision2 = harness.coordinator.submit({ result: publication(2) });
    await waitForSnapshot(harness);

    harness.snapshotRequests[0].resolve(index(2, [beforePath]));
    await vi.waitFor(() => {
      expect(harness.dependencies.prepareGraphProjection).toHaveBeenCalledWith(
        beforePath,
        projectInstanceId,
        expect.any(Number),
      );
    });
    const revision3 = harness.coordinator.submit({
      result: publication(3, { moves: [moveAfter], expectedGraphPaths: [afterPath] }),
    });
    recoveryProjection.resolve(projection(beforePath));

    await expect(revision2).resolves.toMatchObject({ status: 'recovered' });
    const recoveryInput = vi.mocked(harness.dependencies.prepareRecovery).mock.calls[0][0];
    expect([...recoveryInput.pathRemaps]).toEqual([]);

    await expect(revision3).resolves.toMatchObject({ status: 'applied' });
    expect(harness.state.resources).toEqual([afterPath]);
    expect(harness.state.commitOrder).toEqual([3]);
    expect(harness.state.watermark).toBe(3);
  });

  it('rejects only attempt-start entries when snapshot fetch fails', async () => {
    const harness = createHarness();
    const revision2 = harness.coordinator.submit({ result: publication(2) });
    await waitForSnapshot(harness);
    const revision3 = harness.coordinator.submit({ result: publication(3) });
    const revision3Outcome = revision3.then(
      (value) => value,
      (error: unknown) => error,
    );

    harness.snapshotRequests[0].reject(new Error('snapshot N failed'));

    await expect(revision2).rejects.toMatchObject({ code: 'publication_recovery_failed' });
    await waitForSnapshot(harness, 2);
    expect(harness.coordinator.getSnapshotForTests().pendingRevisions).toEqual([3]);
    harness.snapshotRequests[1].resolve(index(3));
    await expect(revision3Outcome).resolves.toMatchObject({ status: 'recovered' });
  });

  it('keeps consecutive unloaded graph creations index-only across recovery attempts', async () => {
    const harness = createHarness();
    harness.state.projections = [];
    const firstPath = 'events/Created-1.yssbi-event';
    const secondPath = 'events/Created-2.yssbi-event';
    let hydrationCount = 0;
    vi.mocked(harness.dependencies.prepareGraphProjection).mockImplementation(async (path) => {
      hydrationCount += 1;
      return hydrationCount === 1 ? projection(path) : false;
    });

    const first = harness.coordinator.submit({
      result: publication(1, { invalidatedGraphPaths: [firstPath] }),
    });
    await waitForSnapshot(harness);
    harness.snapshotRequests[0].resolve(index(1, [firstPath]));
    await expect(first).resolves.toMatchObject({ status: 'recovered' });

    const second = harness.coordinator.submit({
      result: publication(2, { invalidatedGraphPaths: [secondPath] }),
    });
    await waitForSnapshot(harness, 2);
    harness.snapshotRequests[1].resolve(index(2, [firstPath, secondPath]));

    await expect(second).resolves.toMatchObject({ status: 'recovered' });
    expect(harness.dependencies.prepareGraphProjection).not.toHaveBeenCalled();
    expect(harness.state.resources).toEqual([firstPath, secondPath]);
    expect(harness.state.projections).toEqual([]);
    expect(harness.state.watermark).toBe(2);
    expect(harness.dependencies.markProjectProjectionStale).not.toHaveBeenCalled();
  });

  it('keeps later N+1 queued when snapshot N hydration fails and recovers it next', async () => {
    const harness = createHarness();
    const failedHydration = requestProjection(harness, beforePath);
    const revision2 = harness.coordinator.submit({ result: publication(2) });
    await waitForSnapshot(harness);
    harness.snapshotRequests[0].resolve(index(2, [beforePath]));
    await vi.waitFor(() => {
      expect(harness.dependencies.prepareGraphProjection).toHaveBeenCalledWith(
        beforePath,
        projectInstanceId,
        expect.any(Number),
      );
    });

    const futurePath = 'events/Future.yssbi-event';
    const futureHydration = requestProjection(harness, futurePath);
    const revision3 = harness.coordinator.submit({
      result: publication(3, { invalidatedGraphPaths: [futurePath] }),
    });
    const revision3Outcome = revision3.then(
      (value) => value,
      (error: unknown) => error,
    );
    failedHydration.resolve(false);

    await expect(revision2).rejects.toMatchObject({ code: 'publication_recovery_failed' });
    await waitForSnapshot(harness, 2);
    expect(harness.coordinator.getSnapshotForTests().pendingRevisions).toEqual([3]);
    harness.snapshotRequests[1].resolve(index(3, [futurePath]));
    futureHydration.resolve(projection(futurePath));

    await expect(revision3Outcome).resolves.toMatchObject({ status: 'recovered' });
    expect(harness.coordinator.getSnapshotForTests()).toMatchObject({
      appliedRevision: 3,
      phase: 'idle',
      pendingRevisions: [],
    });
  });

  it('assigns an unknown old revision submitted during recovery to the active attempt', async () => {
    const harness = createHarness();
    harness.coordinator.startProject(projectInstanceId, 2);
    const recoveryProjection = requestProjection(harness, beforePath);
    const revision4 = harness.coordinator.submit({ result: publication(4) });
    await waitForSnapshot(harness);

    harness.snapshotRequests[0].resolve(index(4, [beforePath]));
    await vi.waitFor(() => {
      expect(harness.dependencies.prepareGraphProjection).toHaveBeenCalledWith(
        beforePath,
        projectInstanceId,
        expect.any(Number),
      );
    });
    const unknownRevision1 = harness.coordinator.submit({ result: publication(1) });
    recoveryProjection.resolve(projection(beforePath));

    await expect(revision4).resolves.toMatchObject({ status: 'recovered' });
    await expect(unknownRevision1).resolves.toMatchObject({ status: 'recovered' });
    expect(harness.snapshotRequests).toHaveLength(1);
    expect(harness.coordinator.getSnapshotForTests().pendingRevisions).toEqual([]);
  });

  it('rechecks the watermark when missing N arrives during recovery I/O', async () => {
    const harness = createHarness();
    const revision2 = harness.coordinator.submit({ result: publication(2) });
    await waitForSnapshot(harness);
    const revision1 = harness.coordinator.submit({ result: publication(1) });

    harness.snapshotRequests[0].resolve(index(2));

    await expect(revision1).resolves.toMatchObject({ status: 'recovered' });
    await expect(revision2).resolves.toMatchObject({ status: 'recovered' });
    expect(harness.state.commitOrder).toEqual([]);
    expect(harness.state.watermark).toBe(2);
  });

  it('rejects every snapshot-covered waiter and clears pending state when hydration fails', async () => {
    const harness = createHarness();
    const failedHydration = requestProjection(harness, beforePath);
    const first = harness.coordinator.submit({ result: publication(2) });
    await waitForSnapshot(harness);
    harness.snapshotRequests[0].resolve(index(3, [beforePath]));
    await vi.waitFor(() => {
      expect(harness.dependencies.prepareGraphProjection).toHaveBeenCalledWith(
        beforePath,
        projectInstanceId,
        expect.any(Number),
      );
    });
    const second = harness.coordinator.submit({ result: publication(3) });

    failedHydration.resolve(false);

    await expect(first).rejects.toMatchObject({ code: 'publication_recovery_failed' });
    await expect(second).rejects.toMatchObject({ code: 'publication_recovery_failed' });
    expect(harness.coordinator.getSnapshotForTests()).toMatchObject({
      phase: 'idle',
      pendingRevisions: [],
      appliedRevision: 0,
    });
    expect(harness.dependencies.markProjectProjectionStale).toHaveBeenCalledOnce();
  });

  it('permits a later submission to start a fresh recovery after failure', async () => {
    const harness = createHarness();
    const failed = harness.coordinator.submit({ result: publication(2) });
    await waitForSnapshot(harness);
    harness.snapshotRequests[0].reject(new Error('offline'));
    await expect(failed).rejects.toBeInstanceOf(ProjectPublicationError);

    const retry = harness.coordinator.submit({ result: publication(2) });
    await waitForSnapshot(harness, 2);
    harness.snapshotRequests[1].resolve(index(2));

    await expect(retry).resolves.toMatchObject({ status: 'recovered' });
  });

  it('deduplicates matching event delivery after a synchronous direct commit', async () => {
    const harness = createHarness();
    const result = publication(1, { expectedGraphPaths: [] });
    const submission = { result };

    const direct = harness.coordinator.submit(submission);
    const event = harness.coordinator.submit({ result: structuredClone(result) });

    expect(direct).not.toBe(event);
    await expect(Promise.all([direct, event])).resolves.toMatchObject([
      { status: 'applied' },
      { status: 'duplicate' },
    ]);
    expect(harness.dependencies.commitPublication).toHaveBeenCalledOnce();
  });

  it('rejects a different fingerprint at the same revision with publication_protocol_error', async () => {
    const harness = createHarness();
    const first = harness.coordinator.submit({ result: publication(2) });
    await waitForSnapshot(harness);

    const conflicting = harness.coordinator.submit({
      result: publication(2, { history: { canUndo: false, canRedo: true } }),
    });

    await expect(conflicting).rejects.toMatchObject({ code: 'publication_protocol_error' });
    harness.coordinator.cancelProject();
    await expect(first).rejects.toMatchObject({ code: 'stale_project_lifecycle' });
  });

  it('does not preload a loaded move destination already carried by replacements', async () => {
    const harness = createHarness();
    const unexpectedDestination = requestProjection(harness, afterPath);
    const submitted = harness.coordinator.submit({
      result: publication(1, { moves: [moveAfter], expectedGraphPaths: [afterPath] }),
    });
    unexpectedDestination.resolve(false);

    await expect(submitted).resolves.toMatchObject({ status: 'applied' });
    expect(harness.dependencies.prepareGraphProjection).not.toHaveBeenCalled();
    expect(harness.dependencies.prepareMove).toHaveBeenCalledWith(moveAfter, true);
    expect(harness.state.projections).toEqual([afterPath]);
  });

  it('applies an unloaded complete move without preloading or inventing a destination projection', async () => {
    const harness = createHarness();
    harness.state.projections = [];
    const unexpectedDestination = requestProjection(harness, afterPath);
    const submitted = harness.coordinator.submit({
      result: publication(1, {
        moves: [moveAfter],
        expectedGraphPaths: [],
      }),
    });
    unexpectedDestination.resolve(projection(afterPath));

    await expect(submitted).resolves.toMatchObject({ status: 'applied' });
    expect(harness.dependencies.prepareGraphProjection).not.toHaveBeenCalled();
    expect(harness.dependencies.prepareMove).toHaveBeenCalledWith(moveAfter, false);
    expect(harness.state.resources).toEqual([afterPath]);
    expect(harness.state.projections).toEqual([]);
  });

  it('routes incomplete move publication directly to recovery without a normal graph commit', async () => {
    const harness = createHarness();
    harness.state.projections = [];
    const submitted = harness.coordinator.submit({
      result: publication(1, {
        moves: [moveAfter],
        invalidatedGraphPaths: [afterPath],
      }),
    });

    await waitForSnapshot(harness);
    expect(harness.dependencies.prepareGraphProjection).not.toHaveBeenCalled();
    harness.snapshotRequests[0].resolve(index(1, [afterPath]));

    await expect(submitted).resolves.toMatchObject({ status: 'recovered' });
    expect(harness.dependencies.prepareGraphProjection).not.toHaveBeenCalled();
    expect(harness.dependencies.prepareMove).not.toHaveBeenCalled();
    expect(harness.dependencies.commitPublication).not.toHaveBeenCalled();
    expect(harness.state.resources).toEqual([afterPath]);
    expect(harness.state.projections).toEqual([]);
  });

  it('installs a caller replacement without hydrating fallback paths', async () => {
    const harness = createHarness();
    const callerPath = 'events/Caller.yssbi-event';
    const unexpectedCaller = requestProjection(harness, callerPath);
    const submitted = harness.coordinator.submit({
      result: publication(1, { expectedGraphPaths: [callerPath] }),
      fallbackPaths: [callerPath],
    });
    unexpectedCaller.resolve(false);

    await expect(submitted).resolves.toMatchObject({ status: 'applied' });
    expect(harness.dependencies.prepareGraphProjection).not.toHaveBeenCalled();
    expect(harness.state.projections).toEqual([beforePath, callerPath]);
  });

  it('recovers an incomplete move without losing metadata or document flags', async () => {
    const harness = createHarness();
    const submitted = harness.coordinator.submit({
      result: publication(1, { moves: [moveAfter], invalidatedGraphPaths: [afterPath] }),
    });
    await waitForSnapshot(harness);

    const recoveryDestination = requestProjection(harness, afterPath);
    harness.snapshotRequests[0].resolve(index(1, [afterPath]));
    recoveryDestination.resolve(projection(afterPath));

    await expect(submitted).resolves.toMatchObject({ status: 'recovered' });
    expect(harness.state.names[afterPath]).toBe('After');
    expect(harness.state.documentFlags[afterPath]).toEqual({
      dirty: true,
      stale: false,
      conflict: false,
    });
  });

  it('installs authoritative destination names for rename undo and redo', async () => {
    const harness = createHarness();
    const rename = harness.coordinator.submit({
      result: publication(1, { moves: [moveAfter], expectedGraphPaths: [afterPath] }),
    });
    await expect(rename).resolves.toMatchObject({ status: 'applied' });
    expect(harness.state.names).toEqual({ [afterPath]: 'After' });

    const undo = harness.coordinator.submit({
      result: publication(2, { moves: [moveBefore], expectedGraphPaths: [beforePath] }),
    });
    await expect(undo).resolves.toMatchObject({ status: 'applied' });
    expect(harness.state.names).toEqual({ [beforePath]: 'Before' });
  });

  it('recovery ignores a historical move whose destination is absent and source is authoritative', async () => {
    const harness = createHarness();
    const authoritativeProjection = requestProjection(harness, beforePath);
    const submitted = harness.coordinator.submit({
      result: publication(2, { moves: [moveAfter] }),
    });
    await waitForSnapshot(harness);

    harness.snapshotRequests[0].resolve(index(2, [beforePath]));
    authoritativeProjection.resolve(projection(beforePath));

    await expect(submitted).resolves.toMatchObject({ status: 'recovered' });
    const recoveryInput = vi.mocked(harness.dependencies.prepareRecovery).mock.calls[0][0];
    expect([...recoveryInput.pathRemaps]).toEqual([]);
    expect(harness.state.resources).toEqual([beforePath]);
  });

  it('recovery handles rename followed by undo without cycle failure', async () => {
    const harness = createHarness();
    const authoritativeProjection = requestProjection(harness, beforePath);
    const rename = harness.coordinator.submit({
      result: publication(2, { moves: [moveAfter] }),
    });
    await waitForSnapshot(harness);
    const undo = harness.coordinator.submit({
      result: publication(3, { moves: [moveBefore] }),
    });

    harness.snapshotRequests[0].resolve(index(3, [beforePath]));
    authoritativeProjection.resolve(projection(beforePath));

    await expect(rename).resolves.toMatchObject({ status: 'recovered' });
    await expect(undo).resolves.toMatchObject({ status: 'recovered' });
    const recoveryInput = vi.mocked(harness.dependencies.prepareRecovery).mock.calls[0][0];
    expect([...recoveryInput.pathRemaps]).toEqual([[afterPath, beforePath]]);
    expect(harness.state.resources).toEqual([beforePath]);
  });

  it('preserves loaded ownership across a chained rename to the authoritative terminal', async () => {
    const intermediatePath = 'events/Intermediate.yssbi-event';
    const terminalPath = 'events/Terminal.yssbi-event';
    const snapshotRequest = deferred<ProjectIndexRow>();
    const sourceResource = buildGraphResourceMeta('event', beforePath, 'Before');
    useResourceStore.getState().setSnapshot({ resources: [sourceResource], graphOrder: [beforePath] });
    markResourceLoaded({ id: beforePath, kind: 'event' });
    useDocumentStateStore.getState().upsertDocument({
      resourceKey: resourceKey(sourceResource),
      loaded: true,
      dirty: true,
      stale: true,
      missing: false,
      conflict: true,
      version: 7,
    });
    useGraphDataStore.getState().replaceProjection(beforePath, projection(beforePath), 1);
    useGraphMetaStore.getState().addGraph({ path: beforePath, name: 'Before', type: 'event' });
    useGraphSessionStore.getState().setFocusedSession('editor', beforePath);
    useEditorTabStore.getState().initGroupPlacement('editor', [
      { id: beforePath, component: 'GraphEditor', type: 'event' },
    ], beforePath);
    useViewportStore.getState().setViewport({ groupId: 'editor', graphPath: beforePath }, {
      x: 11,
      y: 22,
      scale: 1.5,
    });
    const dependencies: ProjectPublicationDependencies = {
      loadRecoverySnapshot: vi.fn(() => snapshotRequest.promise),
      prepareGraphProjection: vi.fn(async (path) => projection(path)),
      captureLoadedGraphPaths: vi.fn(() => new Set(Object.keys(useGraphDataStore.getState().graphEntities))),
      preparePublication: vi.fn(() => { throw new Error('unexpected normal publication'); }),
      prepareRecovery: vi.fn((plan) => prepareProjectRecoveryCommit(plan)),
      prepareMove: vi.fn(() => { throw new Error('unexpected move preparation'); }),
      commitPublication: vi.fn(),
      commitRecovery: commitPreparedProjectRecovery,
      markProjectProjectionStale: vi.fn(),
    };
    const coordinator = new ProjectPublicationCoordinator(dependencies);
    coordinator.startProject(projectInstanceId, 0);
    const first = coordinator.submit({
      result: publication(2, { moves: [{
        from: beforePath,
        to: intermediatePath,
        kind: 'event',
        name: 'Intermediate',
      }] }),
    });
    await vi.waitFor(() => expect(dependencies.loadRecoverySnapshot).toHaveBeenCalledOnce());
    const second = coordinator.submit({
      result: publication(3, { moves: [{
        from: intermediatePath,
        to: terminalPath,
        kind: 'event',
        name: 'Terminal',
      }] }),
    });

    snapshotRequest.resolve(index(3, [terminalPath]));

    await expect(Promise.all([first, second])).resolves.toMatchObject([
      { status: 'recovered' },
      { status: 'recovered' },
    ]);
    expect(dependencies.prepareGraphProjection).toHaveBeenCalledWith(
      terminalPath,
      projectInstanceId,
      expect.any(Number),
    );
    const recoveryInput = vi.mocked(dependencies.prepareRecovery).mock.calls[0][0];
    expect([...recoveryInput.pathRemaps]).toEqual([
      [beforePath, terminalPath],
      [intermediatePath, terminalPath],
    ]);
    expect(useResourceStore.getState().resources[resourceKey({ id: terminalPath, kind: 'event' })])
      .toMatchObject({
        id: terminalPath,
        loaded: true,
        hasDirtyDocument: true,
        hasStaleDocument: true,
        hasConflictDocument: true,
      });
    expect(useDocumentStateStore.getState().documents[
      resourceKey({ id: terminalPath, kind: 'event' })
    ]).toMatchObject({ loaded: true, dirty: true, stale: true, conflict: true, version: 7 });
    expect(Object.keys(useGraphDataStore.getState().graphEntities)).toEqual([terminalPath]);
    expect(useGraphSessionStore.getState().focusedSession).toEqual({
      groupId: 'editor',
      graphPath: terminalPath,
    });
    expect(useEditorTabStore.getState().getPlacement('editor')).toMatchObject({
      tabIds: [terminalPath],
      selectedTabIds: [terminalPath],
      activeTabId: terminalPath,
    });
    expect(useViewportStore.getState().viewports[
      viewportScopeKey({ groupId: 'editor', graphPath: terminalPath })
    ]).toEqual({ x: 11, y: 22, scale: 1.5 });
  });

  it('recovery remaps only moves consistent with authoritative snapshot', async () => {
    const harness = createHarness();
    const discardedSource = 'events/Discarded.yssbi-event';
    const discardedDestination = 'events/Absent.yssbi-event';
    const authoritativeProjection = requestProjection(harness, afterPath);
    const submitted = harness.coordinator.submit({
      result: publication(2, {
        moves: [
          moveAfter,
          {
            from: discardedSource,
            to: discardedDestination,
            kind: 'event',
            name: 'Absent',
          },
        ],
      }),
    });
    await waitForSnapshot(harness);

    harness.snapshotRequests[0].resolve(index(2, [afterPath]));
    authoritativeProjection.resolve(projection(afterPath));

    await expect(submitted).resolves.toMatchObject({ status: 'recovered' });
    const recoveryInput = vi.mocked(harness.dependencies.prepareRecovery).mock.calls[0][0];
    expect([...recoveryInput.pathRemaps]).toEqual([[beforePath, afterPath]]);
    expect(harness.state.resources).toEqual([afterPath]);
  });

  it('stale snapshot rejects attempt without automatic retry', async () => {
    const harness = createHarness();
    harness.coordinator.startProject(projectInstanceId, 2);
    const submitted = harness.coordinator.submit({ result: publication(4) });
    const outcomePromise = submitted.then(
      (value) => value,
      (error: unknown) => error,
    );
    await waitForSnapshot(harness);

    harness.snapshotRequests[0].resolve(index(1));
    const outcome = await Promise.race([
      outcomePromise,
      new Promise<'timeout'>((resolve) => setTimeout(() => resolve('timeout'), 100)),
    ]);
    if (outcome === 'timeout') harness.coordinator.cancelProject();

    expect(outcome).toMatchObject({ code: 'publication_recovery_failed' });
    expect(harness.snapshotRequests).toHaveLength(1);
    expect(harness.dependencies.markProjectProjectionStale).toHaveBeenCalledOnce();
    expect(harness.coordinator.getSnapshotForTests()).toMatchObject({
      appliedRevision: 2,
      phase: 'idle',
      pendingRevisions: [],
    });
  });

  it('non-advancing snapshot rejects owned waiters and clears recovery state', async () => {
    const harness = createHarness();
    harness.coordinator.startProject(projectInstanceId, 2);
    const submitted = harness.coordinator.submit({ result: publication(4) });
    const outcomePromise = submitted.then(
      (value) => value,
      (error: unknown) => error,
    );
    await waitForSnapshot(harness);

    harness.snapshotRequests[0].resolve(index(2));
    const outcome = await Promise.race([
      outcomePromise,
      new Promise<'timeout'>((resolve) => setTimeout(() => resolve('timeout'), 100)),
    ]);
    if (outcome === 'timeout') harness.coordinator.cancelProject();

    expect(outcome).toMatchObject({ code: 'publication_recovery_failed' });
    expect(harness.dependencies.commitRecovery).not.toHaveBeenCalled();
    expect(harness.dependencies.markProjectProjectionStale).toHaveBeenCalledOnce();
    expect(harness.coordinator.getSnapshotForTests()).toMatchObject({
      appliedRevision: 2,
      phase: 'idle',
      pendingRevisions: [],
    });
  });

  it('later fresh submission can retry after non-advancing snapshot failure', async () => {
    const harness = createHarness();
    harness.coordinator.startProject(projectInstanceId, 2);
    const failed = harness.coordinator.submit({ result: publication(4) });
    await waitForSnapshot(harness);
    harness.snapshotRequests[0].resolve(index(2));
    await expect(failed).rejects.toMatchObject({ code: 'publication_recovery_failed' });

    const retry = harness.coordinator.submit({ result: publication(4) });
    await waitForSnapshot(harness, 2);
    harness.snapshotRequests[1].resolve(index(4));

    await expect(retry).resolves.toMatchObject({ status: 'recovered' });
    expect(harness.coordinator.getSnapshotForTests()).toMatchObject({
      appliedRevision: 4,
      phase: 'idle',
      pendingRevisions: [],
    });
  });

  it('recovers resources functions projections history and watermark from one snapshot', async () => {
    const harness = createHarness();
    const functionPath = 'functions/Calculate.yssbi-function';
    const submitted = harness.coordinator.submit({
      result: publication(2, { invalidatedGraphPaths: [functionPath] }),
    });
    await waitForSnapshot(harness);
    harness.snapshotRequests[0].resolve(index(2, [], {
      graphs: [{
        path: functionPath,
        name: 'Calculate',
        type: 'function',
        revision: 7,
        functionRevision: 7,
        functionSignature: { parameters: [], return_type: 'Int64' },
        functionEditorProjection: {
          functionRevision: 7,
          inputs: [],
          outputs: [{ id: 'return', name: 'Int64', dataType: { kind: 'Int64' } }],
        },
      }],
      history: { canUndo: true, canRedo: true },
    }));

    await expect(submitted).resolves.toMatchObject({ status: 'recovered' });
    expect(harness.state).toMatchObject({
      resources: [functionPath],
      projections: [],
      functionRevisions: { [functionPath]: 7 },
      history: { canUndo: true, canRedo: true },
      watermark: 2,
    });
  });

  it('settles revisions at or below the recovered watermark without replay', async () => {
    const harness = createHarness();
    const revision2 = harness.coordinator.submit({ result: publication(2) });
    const revision3 = harness.coordinator.submit({ result: publication(3) });
    await waitForSnapshot(harness);
    harness.snapshotRequests[0].resolve(index(3));

    await expect(revision2).resolves.toMatchObject({ status: 'recovered' });
    await expect(revision3).resolves.toMatchObject({ status: 'recovered' });
    expect(harness.dependencies.commitPublication).not.toHaveBeenCalled();
    expect(harness.state.watermark).toBe(3);
  });

  it('does not regress a watermark advanced while recovery I/O is pending', async () => {
    const harness = createHarness();
    const revision2 = harness.coordinator.submit({
      result: publication(2, { expectedGraphPaths: [] }),
    });
    await waitForSnapshot(harness);
    const revision1 = harness.coordinator.submit({ result: publication(1) });

    harness.snapshotRequests[0].resolve(index(1));
    await expect(revision1).resolves.toMatchObject({ status: 'recovered' });
    await expect(revision2).resolves.toMatchObject({ status: 'applied' });
    await vi.waitFor(() => {
      expect(harness.coordinator.getSnapshotForTests().phase).toBe('idle');
    });

    expect(harness.coordinator.getSnapshotForTests().appliedRevision).toBe(2);
    expect(harness.state.watermark).toBe(2);
  });

  it('recovers before committing when a replacement has malformed nested projection identity', async () => {
    const harness = createHarness();
    const before = structuredClone(harness.state);
    const result = publication(1, {
      moves: [moveAfter],
      expectedGraphPaths: [afterPath],
    });
    result.projectionReplacements[0].projection.nodes[0].graphPath = beforePath;
    const submitted = harness.coordinator.submit({ result });

    await waitForSnapshot(harness);
    expect(harness.state).toEqual(before);

    harness.snapshotRequests[0].reject(new Error('stop recovery'));
    await expect(submitted).rejects.toMatchObject({ code: 'publication_recovery_failed' });
  });

  it('does not let a rejected old recovery snapshot mutate the replacement recovery lifecycle', async () => {
    const harness = createHarness();
    installRealStaleMarker(harness);
    const oldWaiter = harness.coordinator.submit({ result: publication(2) });
    await waitForSnapshot(harness);

    harness.coordinator.startProject(replacementProjectInstanceId, 0);
    await expect(oldWaiter).rejects.toMatchObject({ code: 'stale_project_lifecycle' });
    const replacementPath = 'events/Replacement.yssbi-event';
    const storesBefore = installReplacementStoreBaseline(replacementPath);
    const replacementResult = publication(2);
    replacementResult.projectInstanceId = replacementProjectInstanceId;
    const replacementWaiter = harness.coordinator.submit({ result: replacementResult });
    await waitForSnapshot(harness, 2);
    const lifecycleBefore = harness.coordinator.getSnapshotForTests();

    harness.snapshotRequests[0].reject(new Error('old snapshot failed after replacement'));
    await flushRejectedRecovery();

    expect(harness.dependencies.markProjectProjectionStale).not.toHaveBeenCalled();
    expect(harness.coordinator.getSnapshotForTests()).toEqual(lifecycleBefore);
    expect(useResourceStore.getState().resources).toEqual(storesBefore.resources);
    expect(useDocumentStateStore.getState().documents).toEqual(storesBefore.documents);

    harness.snapshotRequests[1].resolve(index(2, [], {
      projectInstanceId: replacementProjectInstanceId,
    }));
    await expect(replacementWaiter).resolves.toMatchObject({ status: 'recovered' });
  });

  it('does not let a rejected old recovery projection mutate the replacement recovery lifecycle', async () => {
    const harness = createHarness();
    installRealStaleMarker(harness);
    const oldProjection = requestProjection(harness, beforePath);
    const oldWaiter = harness.coordinator.submit({ result: publication(2) });
    await waitForSnapshot(harness);
    harness.snapshotRequests[0].resolve(index(2, [beforePath]));
    await vi.waitFor(() => {
      expect(harness.dependencies.prepareGraphProjection).toHaveBeenCalledWith(
        beforePath,
        projectInstanceId,
        expect.any(Number),
      );
    });

    harness.coordinator.startProject(replacementProjectInstanceId, 0);
    await expect(oldWaiter).rejects.toMatchObject({ code: 'stale_project_lifecycle' });
    const replacementPath = 'events/Replacement.yssbi-event';
    const storesBefore = installReplacementStoreBaseline(replacementPath);
    const replacementResult = publication(2);
    replacementResult.projectInstanceId = replacementProjectInstanceId;
    const replacementWaiter = harness.coordinator.submit({ result: replacementResult });
    await waitForSnapshot(harness, 2);
    const lifecycleBefore = harness.coordinator.getSnapshotForTests();

    oldProjection.reject(new Error('old projection failed after replacement'));
    await flushRejectedRecovery();

    expect(harness.dependencies.markProjectProjectionStale).not.toHaveBeenCalled();
    expect(harness.coordinator.getSnapshotForTests()).toEqual(lifecycleBefore);
    expect(useResourceStore.getState().resources).toEqual(storesBefore.resources);
    expect(useDocumentStateStore.getState().documents).toEqual(storesBefore.documents);

    harness.snapshotRequests[1].resolve(index(2, [], {
      projectInstanceId: replacementProjectInstanceId,
    }));
    await expect(replacementWaiter).resolves.toMatchObject({ status: 'recovered' });
  });

  it('recovers worksheet moves without requesting graph projection hydration', async () => {
    const harness = createHarness();
    const from = 'opaque worksheet coordinator::before';
    const to = 'opaque worksheet coordinator::after';
    const result: ResourceMutationResultDto = {
      operationId: '00000000-0000-0000-0000-000000000124',
      projectInstanceId,
      publicationRevision: 1,
      moves: [{ from, to, kind: 'worksheet', name: 'After' }],
      deltas: [{
        resource: { kind: 'worksheet', key: to },
        fromRevision: 3,
        toRevision: 4,
        causedBy: null,
        payload: { kind: 'resource_move', patch: { from, to } },
      }],
      projectionReplacements: [],
      projectionStatus: { status: 'incomplete', invalidatedGraphPaths: [] },
      history: { canUndo: true, canRedo: false },
    };

    const submitted = harness.coordinator.submit({ result });
    await waitForSnapshot(harness);
    harness.snapshotRequests[0].resolve(index(1, [], {
      worksheets: [{
        worksheetPath: to,
        name: 'After',
        databaseId: 'database-1',
        chartType: 'line',
        revision: 4,
      }],
    }));

    await expect(submitted).resolves.toMatchObject({ status: 'recovered' });
    const preparation = vi.mocked(harness.dependencies.prepareRecovery).mock.calls[0][0];
    expect([...preparation.pathRemaps]).toEqual([]);
    expect([...(preparation.worksheetPathRemaps ?? [])]).toEqual([[from, to]]);
    expect(harness.dependencies.prepareGraphProjection).not.toHaveBeenCalled();
  });

  it('rejects queued and recovering work when the project lifecycle changes', async () => {
    const harness = createHarness();
    const recovering = harness.coordinator.submit({ result: publication(2) });
    const queued = harness.coordinator.submit({ result: publication(3) });
    await waitForSnapshot(harness);

    harness.coordinator.startProject(
      '00000000-0000-0000-0000-000000000802',
      4,
    );

    await expect(recovering).rejects.toMatchObject({ code: 'stale_project_lifecycle' });
    await expect(queued).rejects.toMatchObject({ code: 'stale_project_lifecycle' });
    expect(harness.coordinator.getSnapshotForTests()).toMatchObject({
      projectInstanceId: '00000000-0000-0000-0000-000000000802',
      appliedRevision: 4,
      phase: 'idle',
      pendingRevisions: [],
    });
  });
});
