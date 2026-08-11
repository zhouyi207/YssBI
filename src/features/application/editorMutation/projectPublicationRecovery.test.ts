import { expect, it } from 'vitest';
import type { ProjectIndexRow } from '@/services/project/projectService';
import { resourceKey } from '@/features/core/resource';
import {
  commitPreparedProjectRecovery,
  prepareProjectRecoveryCommit,
} from './projectPublicationRecovery';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { useResourceStore, useDocumentStateStore } from '@/features/core/resource';
import { useEditorTabStore } from '@/features/core/layout/editorTabStore';
import { useEditorStore } from '@/features/core/editor/stores/useEditorStore';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import {
  clearWorksheetPreviewCache,
  getCachedWorksheetPreview,
  getWorksheetPreview,
} from '@/services/worksheet/worksheetPreviewCache';

it('prepares recovery with authoritative function editor projection pins', () => {
  const functionPath = 'functions/Model.yssbi-function';
  const projectInstanceId = '00000000-0000-0000-0000-000000000701';
  const index: ProjectIndexRow = {
    projectInstanceId,
    projectName: 'Recovered',

    exportTime: '',
    publicationRevision: 4,
    history: { canUndo: true, canRedo: false },
    graphs: [{
      path: functionPath,
      name: 'Model',
      type: 'function',
      revision: 6,
      functionRevision: 6,
      functionSignature: { parameters: [], return_type: 'Object' },
      functionEditorProjection: {
        functionRevision: 6,
        inputs: [],
        outputs: [{
          id: 'computed',
          name: 'Computed value',
          dataType: { kind: 'Struct', inner: 'RegressionModel' },
        }],
      },
    }],
    variables: [],
    worksheets: [],
    databases: [],
  };

  const prepared = prepareProjectRecoveryCommit({
    projectInstanceId,
    epoch: 1,
    publicationRevision: 4,
    index,
    projections: new Map(),
    graphPathsLoadedAtStart: new Set(),
    pathRemaps: new Map(),
  });

  expect(prepared.storeState.resources[
    resourceKey({ id: functionPath, kind: 'function' })
  ]).toMatchObject({ revision: 6 });
  expect(prepared.storeState.graphMeta[functionPath]).toMatchObject({
    functionRevision: 6,
    functionSignature: { parameters: [], return_type: 'Object' },
    functionInputs: [],
    functionOutputs: [{
      id: 'computed',
      name: 'Computed value',
      dataType: { kind: 'Struct', inner: 'RegressionModel' },
    }],
  });
});

it('preserves a graph projection loaded while recovery was in flight', () => {
  const graphPath = 'events/Opened-During-Recovery.yssbi-event';
  const projectInstanceId = '00000000-0000-0000-0000-000000000703';
  const projection = makeEditorProjectionFixture({ graphPath, sourceRevision: 3 }).projection;
  useGraphDataStore.setState({ graphEntities: {} });
  useGraphDataStore.getState().replaceProjection(graphPath, projection, 1);

  const prepared = prepareProjectRecoveryCommit({
    projectInstanceId,
    epoch: 1,
    publicationRevision: 2,
    index: {
      projectInstanceId,
      projectName: 'Recovered',
      exportTime: '',
      publicationRevision: 2,
      history: { canUndo: false, canRedo: false },
      graphs: [{ path: graphPath, name: 'Opened', type: 'event', revision: 3 }],
      variables: [],
      worksheets: [],
      databases: [],
    },
    projections: new Map(),
    graphPathsLoadedAtStart: new Set(),
    pathRemaps: new Map(),
  });

  expect(prepared.graphProjectionPlan.graphEntities[graphPath]).toBe(
    useGraphDataStore.getState().graphEntities[graphPath],
  );
});

it('recovers an opaque worksheet move with document flags tabs and detail focus', () => {
  const projectInstanceId = '00000000-0000-0000-0000-000000000702';
  const from = 'opaque worksheet recovery::before';
  const to = 'opaque worksheet recovery::after';
  const document = {
    schemaVersion: 1,
    revision: 4,
    databaseId: 'database-1',
    chartType: 'line' as const,
    encodings: { x: 'x', y: 'y' },
  };
  const fromKey = resourceKey({ id: from, kind: 'worksheet' });
  useResourceStore.getState().clear();
  useDocumentStateStore.getState().clear();
  useWorksheetStore.setState({
    index: [{
      worksheetPath: from,
      name: 'Before',
      databaseId: 'database-1',
      chartType: 'line',
      revision: document.revision,
    }],
    documents: { [from]: document },
  });
  useResourceStore.getState().upsertResource({
    id: from,
    kind: 'worksheet',
    name: 'Before',
    uri: fromKey,
    exists: true,
    loaded: true,
    hasDirtyDocument: true,
    hasStaleDocument: true,
    hasConflictDocument: true,
  });
  useDocumentStateStore.getState().upsertDocument({
    resourceKey: fromKey,
    loaded: true,
    dirty: true,
    stale: true,
    missing: false,
    conflict: true,
    version: 9,
  });
  useEditorTabStore.setState({ registry: {}, placements: {} });
  useEditorTabStore.getState().initGroupPlacement('editor', [{
    id: from,
    component: 'WorksheetEditor',
    type: 'worksheet',
  }], from);
  useEditorTabStore.getState().setSelectedTabIds('editor', [from]);
  useEditorStore.getState().setDetailFocus({ kind: 'worksheet', worksheetPath: from });
  const index: ProjectIndexRow = {
    projectInstanceId,
    projectName: 'Recovered',
    exportTime: '',
    publicationRevision: 5,
    history: { canUndo: true, canRedo: false },
    graphs: [],
    variables: [],
    worksheets: [{
      worksheetPath: to,
      name: 'After',
      databaseId: 'database-1',
      chartType: 'line',
      revision: 5,
    }],
    databases: [],
  };

  const prepared = prepareProjectRecoveryCommit({
    projectInstanceId,
    epoch: 1,
    publicationRevision: 5,
    index,
    projections: new Map(),
    graphPathsLoadedAtStart: new Set(),
    pathRemaps: new Map(),
    worksheetPathRemaps: new Map([[from, to]]),
  } as Parameters<typeof prepareProjectRecoveryCommit>[0] & {
    worksheetPathRemaps: ReadonlyMap<string, string>;
  });
  commitPreparedProjectRecovery(prepared);

  expect(useWorksheetStore.getState()).toMatchObject({
    index: [{ worksheetPath: to, name: 'After' }],
    documents: { [to]: document },
  });
  expect(useDocumentStateStore.getState().documents[
    resourceKey({ id: to, kind: 'worksheet' })
  ]).toMatchObject({ dirty: true, stale: true, conflict: true, version: 9 });
  expect(useResourceStore.getState().resources[
    resourceKey({ id: to, kind: 'worksheet' })
  ]).toMatchObject({
    id: to,
    name: 'After',
    hasDirtyDocument: true,
    hasStaleDocument: true,
    hasConflictDocument: true,
  });
  expect(useEditorTabStore.getState().getPlacement('editor')).toMatchObject({
    tabIds: [to],
    selectedTabIds: [to],
    activeTabId: to,
  });
  expect(useEditorStore.getState().detailFocus).toEqual({
    kind: 'worksheet',
    worksheetPath: to,
  });
});

it('recovers an A to B to C worksheet move chain from duplicate historical owners', async () => {
  const projectInstanceId = '00000000-0000-0000-0000-000000000704';
  const [pathA, pathB, pathC] = [
    'opaque worksheet chain::A',
    'opaque worksheet chain::B',
    'opaque worksheet chain::C',
  ];
  const documentA = {
    schemaVersion: 1,
    revision: 2,
    databaseId: 'database-1',
    chartType: 'line' as const,
    encodings: { x: 'old-x', y: 'old-y' },
  };
  const documentB = {
    ...documentA,
    revision: 4,
    encodings: { x: 'current-x', y: 'current-y' },
  };
  useResourceStore.getState().clear();
  useDocumentStateStore.getState().clear();
  useGraphDataStore.setState({ graphEntities: {} });
  useWorksheetStore.setState({
    index: [
      { worksheetPath: pathA, name: 'A', databaseId: 'database-1', chartType: 'line', revision: 2 },
      { worksheetPath: pathB, name: 'B', databaseId: 'database-1', chartType: 'line', revision: 4 },
    ],
    documents: { [pathA]: documentA, [pathB]: documentB },
  });
  for (const [path, name, dirty, stale, conflict, version] of [
    [pathA, 'A', false, true, false, 2],
    [pathB, 'B', true, false, true, 7],
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
  clearWorksheetPreviewCache();
  await Promise.all([pathA, pathB, pathC].map((path) =>
    getWorksheetPreview(projectInstanceId, path, documentB, async () => ({ kind: 'empty' }))));
  const graphStateBefore = structuredClone(useGraphDataStore.getState().graphEntities);
  const index: ProjectIndexRow = {
    projectInstanceId,
    projectName: 'Recovered chain',
    exportTime: '',
    publicationRevision: 8,
    history: { canUndo: true, canRedo: true },
    graphs: [],
    variables: [],
    worksheets: [{
      worksheetPath: pathC,
      name: 'C',
      databaseId: 'database-1',
      chartType: 'line',
      revision: 5,
    }],
    databases: [],
  };

  const prepared = prepareProjectRecoveryCommit({
    projectInstanceId,
    epoch: 1,
    publicationRevision: 8,
    index,
    projections: new Map(),
    graphPathsLoadedAtStart: new Set(),
    pathRemaps: new Map(),
    worksheetPathRemaps: new Map([[pathA, pathC], [pathB, pathC]]),
  });
  commitPreparedProjectRecovery(prepared);

  expect(useWorksheetStore.getState()).toMatchObject({
    index: [{ worksheetPath: pathC, name: 'C' }],
    documents: { [pathC]: documentB },
  });
  expect(useDocumentStateStore.getState().documents[
    resourceKey({ id: pathC, kind: 'worksheet' })
  ]).toMatchObject({ dirty: true, stale: false, conflict: true, version: 7 });
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
  for (const path of [pathA, pathB, pathC]) {
    expect(getCachedWorksheetPreview(projectInstanceId, path, documentB)).toBeUndefined();
  }
  expect(useGraphDataStore.getState().graphEntities).toEqual(graphStateBefore);
  expect(prepared.projections.size).toBe(0);
});

it('clears worksheet detail focus absent from a committed authoritative recovery', () => {
  const projectInstanceId = '00000000-0000-0000-0000-000000000703';
  const removedPath = 'worksheets/Removed During Recovery.yssbi-worksheet';
  useEditorStore.getState().setDetailFocus({
    kind: 'worksheet',
    worksheetPath: removedPath,
  });
  const index: ProjectIndexRow = {
    projectInstanceId,
    projectName: 'Recovered removal',
    exportTime: '',
    publicationRevision: 6,
    history: { canUndo: true, canRedo: false },
    graphs: [],
    variables: [],
    worksheets: [],
    databases: [],
  };

  const prepared = prepareProjectRecoveryCommit({
    projectInstanceId,
    epoch: 1,
    publicationRevision: 6,
    index,
    projections: new Map(),
    graphPathsLoadedAtStart: new Set(),
    pathRemaps: new Map(),
  });
  expect(useEditorStore.getState().detailFocus).toEqual({
    kind: 'worksheet',
    worksheetPath: removedPath,
  });

  commitPreparedProjectRecovery(prepared);

  expect(useEditorStore.getState().detailFocus).toBeNull();
});
