import { beforeEach, describe, expect, it } from 'vitest';
import { useGraphDataStore, useGraphMetaStore } from '@/features/core/dataStore';
import {
  resourceKey,
  useDocumentStateStore,
  useResourceStore,
} from '@/features/core/resource';
import type { ResourceMutationResultDto } from '@/shared/types/dto/editorMutation';
import {
  commitPreparedPublication,
  fingerprintResourceMutationResult,
  prepareSynchronousPublicationCommit,
  validateResourceMutationWireResult,
} from './resourceMutationResult';
import { collectProjectRecoveryGraphPaths } from './projectPublicationRecovery';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { useViewportStore } from '@/features/core/viewport';
import { useEditorStore } from '@/features/core/editor/stores/useEditorStore';
import { useEditorTabStore } from '@/features/core/layout/editorTabStore';

const projectInstanceId = '00000000-0000-0000-0000-000000000911';
const operationId = '00000000-0000-0000-0000-000000000912';
const graphPath = 'events/Created.yssbi-event';

function lifecycleResult(
  publicationRevision: number,
  before: { revision: number; path: string; kind: 'event' | 'function'; name: string } | null,
  after: { revision: number; path: string; kind: 'event' | 'function'; name: string } | null,
  path = graphPath,
): ResourceMutationResultDto {
  const fromRevision = before?.revision ?? after?.revision ?? 0;
  const toRevision = after?.revision ?? fromRevision + 1;
  return {
    operationId,
    projectInstanceId,
    publicationRevision,
    moves: [],
    deltas: [{
      resource: { kind: 'graph', key: path },
      fromRevision,
      toRevision,
      causedBy: operationId,
      payload: {
        kind: 'resource_lifecycle',
        patch: { before, after },
      },
    }],
    projectionReplacements: [],
    projectionStatus: { status: 'complete', expectedGraphPaths: [] },
    history: { canUndo: false, canRedo: false },
  } as unknown as ResourceMutationResultDto;
}

const present = { revision: 0, path: graphPath, kind: 'event' as const, name: 'Created' };

function resetStores(): void {
  useGraphDataStore.setState({ graphEntities: {} });
  useGraphMetaStore.setState({ graphs: {} });
  useResourceStore.getState().clear();
  useDocumentStateStore.getState().clear();
  useWorksheetStore.getState().clear();
  useGraphSessionStore.getState().reset();
  useViewportStore.getState().clear();
  useEditorStore.setState({ detailFocus: null });
  useEditorTabStore.setState({ registry: {}, placements: {} });
}

describe('graph resource lifecycle publication', () => {
  beforeEach(resetStores);

  it('validates exact lifecycle identity and includes it in the fingerprint', () => {
    const created = lifecycleResult(1, null, present);
    expect(validateResourceMutationWireResult(created)).toBeUndefined();

    const mismatched = structuredClone(created) as ResourceMutationResultDto;
    (mismatched.deltas[0].payload as unknown as { patch: { after: typeof present } })
      .patch.after.path = 'events/Other.yssbi-event';
    expect(validateResourceMutationWireResult(mismatched)).toContain('resource deltas');
    expect(fingerprintResourceMutationResult(mismatched))
      .not.toBe(fingerprintResourceMutationResult(created));
  });

  it.each([
    'events/Main.yssbi-function',
    'events/../Main.yssbi-event',
    'events//Main.yssbi-event',
    'functions/Main.yssbi-event',
  ])('rejects malformed result move identity %j', (from) => {
    const result = lifecycleResult(1, null, present);
    result.moves = [{
      from,
      to: 'events/Renamed.yssbi-event',
      kind: 'event',
      name: 'Renamed',
    }];

    expect(validateResourceMutationWireResult(result)).toBe('resource moves are malformed');
  });

  it('accepts nested event and function identities in resource results', () => {
    const eventPath = 'events/folder/sub-folder/Main.v2.yssbi-event';
    const functionPath = 'functions/library/math/Calculate.yssbi-function';

    expect(validateResourceMutationWireResult(lifecycleResult(
      1,
      null,
      { revision: 0, path: eventPath, kind: 'event', name: 'Main.v2' },
      eventPath,
    ))).toBeUndefined();
    expect(validateResourceMutationWireResult(lifecycleResult(
      1,
      null,
      { revision: 0, path: functionPath, kind: 'function', name: 'Calculate' },
      functionPath,
    ))).toBeUndefined();
  });

  it('applies canonical create and remove lifecycle deltas to production stores', () => {
    const created = lifecycleResult(1, null, present);
    const createPlan = prepareSynchronousPublicationCommit(created, {
      projectInstanceId,
      epoch: 1,
      fingerprint: fingerprintResourceMutationResult(created),
      affectedGraphPaths: new Set([graphPath]),
      moves: [],
    });
    commitPreparedPublication(createPlan);

    const key = resourceKey({ id: graphPath, kind: 'event' });
    expect(useResourceStore.getState().resources[key]).toMatchObject({
      id: graphPath,
      kind: 'event',
      name: 'Created',
      revision: 0,
      loaded: false,
    });
    expect(useResourceStore.getState().graphOrder).toEqual([graphPath]);
    expect(useGraphMetaStore.getState().graphs[graphPath]).toEqual({
      path: graphPath,
      name: 'Created',
      type: 'event',
    });

    const removed = lifecycleResult(2, present, null);
    const removePlan = prepareSynchronousPublicationCommit(removed, {
      projectInstanceId,
      epoch: 1,
      fingerprint: fingerprintResourceMutationResult(removed),
      affectedGraphPaths: new Set([graphPath]),
      moves: [],
    });
    commitPreparedPublication(removePlan);

    expect(useResourceStore.getState().resources[key]).toBeUndefined();
    expect(useResourceStore.getState().graphOrder).toEqual([]);
    expect(useGraphMetaStore.getState().graphs[graphPath]).toBeUndefined();
  });

  it('removes a loaded graph using projection authority when sidebar metadata is stale', () => {
    const created = lifecycleResult(1, null, present);
    commitPreparedPublication(prepareSynchronousPublicationCommit(created, {
      projectInstanceId,
      epoch: 1,
      fingerprint: fingerprintResourceMutationResult(created),
      affectedGraphPaths: new Set([graphPath]),
      moves: [],
    }));
    useGraphDataStore.getState().replaceProjection(
      graphPath,
      makeEditorProjectionFixture({ graphPath, sourceRevision: 2 }).projection,
      1,
    );

    const removedState = { ...present, revision: 2 };
    const removed = lifecycleResult(2, removedState, null);
    const removePlan = prepareSynchronousPublicationCommit(removed, {
      projectInstanceId,
      epoch: 1,
      fingerprint: fingerprintResourceMutationResult(removed),
      affectedGraphPaths: new Set([graphPath]),
      moves: [],
    });
    commitPreparedPublication(removePlan);

    expect(useResourceStore.getState().resources[
      resourceKey({ id: graphPath, kind: 'event' })
    ]).toBeUndefined();
  });

  it('prepares a worksheet lifecycle insert as one index resource and document-state commit', () => {
    const worksheetPath = 'opaque worksheet identity::created';
    const document = {
      schemaVersion: 1,
      revision: 0,
      databaseId: 'database-1',
      chartType: 'scatter' as const,
      encodings: { x: 'amount', y: 'count' },
    };
    useWorksheetStore.setState({ documents: { [worksheetPath]: document } });
    const result: ResourceMutationResultDto = {
      operationId,
      projectInstanceId,
      publicationRevision: 1,
      moves: [],
      deltas: [{
        resource: { kind: 'worksheet', key: worksheetPath },
        fromRevision: 0,
        toRevision: 0,
        causedBy: operationId,
        payload: {
          kind: 'resource_lifecycle',
          patch: {
            before: null,
            after: { revision: 0, path: worksheetPath, kind: 'worksheet', name: 'Revenue' },
          },
        },
      }],
      projectionReplacements: [],
      projectionStatus: { status: 'complete', expectedGraphPaths: [] },
      history: { canUndo: true, canRedo: false },
    };

    const notifications = { data: 0, meta: 0, session: 0, viewport: 0 };
    const unsubscribers = [
      useGraphDataStore.subscribe(() => { notifications.data += 1; }),
      useGraphMetaStore.subscribe(() => { notifications.meta += 1; }),
      useGraphSessionStore.subscribe(() => { notifications.session += 1; }),
      useViewportStore.subscribe(() => { notifications.viewport += 1; }),
    ];
    const plan = prepareSynchronousPublicationCommit(result, {
      projectInstanceId,
      epoch: 1,
      fingerprint: fingerprintResourceMutationResult(result),
      affectedGraphPaths: new Set(),
      moves: [],
    });
    commitPreparedPublication(plan);
    unsubscribers.forEach((unsubscribe) => unsubscribe());

    expect(plan.graphProjectionPlan).toBeUndefined();
    expect(plan.storeState).not.toHaveProperty('graphMeta');
    expect(plan.storeState).not.toHaveProperty('focusedSession');
    expect(plan.storeState).not.toHaveProperty('viewports');
    expect(notifications).toEqual({ data: 0, meta: 0, session: 0, viewport: 0 });
    expect(plan.storeState.worksheetIndex).toEqual([{
      worksheetPath,
      name: 'Revenue',
      databaseId: 'database-1',
      chartType: 'scatter',
      revision: 0,
    }]);
    expect(plan.storeState.resources[
      resourceKey({ id: worksheetPath, kind: 'worksheet' })
    ]).toMatchObject({ id: worksheetPath, name: 'Revenue', loaded: true });
    expect(plan.storeState.documents[
      resourceKey({ id: worksheetPath, kind: 'worksheet' })
    ]).toMatchObject({ loaded: true });
  });

  it('clears matching worksheet detail focus only when removal publication commits', () => {
    const worksheetPath = 'worksheets/Focused.yssbi-worksheet';
    const document = {
      schemaVersion: 1,
      revision: 0,
      databaseId: 'database-1',
      chartType: 'scatter' as const,
      encodings: {},
    };
    useWorksheetStore.setState({
      index: [{
        worksheetPath,
        name: 'Focused worksheet',
        databaseId: document.databaseId,
        chartType: document.chartType,
        revision: document.revision,
      }],
      documents: { [worksheetPath]: document },
    });
    useResourceStore.getState().upsertResource({
      id: worksheetPath,
      kind: 'worksheet',
      name: 'Focused worksheet',
      uri: resourceKey({ id: worksheetPath, kind: 'worksheet' }),
      revision: 0,
      exists: true,
      loaded: true,
      hasDirtyDocument: false,
      hasStaleDocument: false,
      hasConflictDocument: false,
    });
    useEditorTabStore.getState().initGroupPlacement('editor', [{
      id: worksheetPath,
      type: 'worksheet',
      component: 'WorksheetEditor',
    }], worksheetPath);
    useEditorStore.getState().setDetailFocus({ kind: 'worksheet', worksheetPath });
    const result: ResourceMutationResultDto = {
      operationId,
      projectInstanceId,
      publicationRevision: 1,
      moves: [],
      deltas: [{
        resource: { kind: 'worksheet', key: worksheetPath },
        fromRevision: 0,
        toRevision: 1,
        causedBy: operationId,
        payload: {
          kind: 'resource_lifecycle',
          patch: {
            before: {
              revision: 0,
              path: worksheetPath,
              kind: 'worksheet',
              name: 'Focused worksheet',
            },
            after: null,
          },
        },
      }],
      projectionReplacements: [],
      projectionStatus: { status: 'complete', expectedGraphPaths: [] },
      history: { canUndo: true, canRedo: false },
    };

    const plan = prepareSynchronousPublicationCommit(result, {
      projectInstanceId,
      epoch: 1,
      fingerprint: fingerprintResourceMutationResult(result),
      affectedGraphPaths: new Set(),
      moves: [],
    });

    expect(useEditorStore.getState().detailFocus).toEqual({
      kind: 'worksheet',
      worksheetPath,
    });
    commitPreparedPublication(plan);
    expect(useEditorStore.getState().detailFocus).toBeNull();
  });

  it('uses lifecycle payload paths when selecting recovery hydration', () => {
    const created = lifecycleResult(2, null, present);
    const paths = collectProjectRecoveryGraphPaths(
      {
        projectInstanceId,
        projectName: 'Current',

        exportTime: '',
        publicationRevision: 2,
        history: { canUndo: false, canRedo: false },
        graphs: [{ path: graphPath, name: 'Created', type: 'event', revision: 0 }],
        variables: [],
        worksheets: [],
        databases: [],
      },
      new Set(),
      [created],
    );

    expect(paths).toEqual(new Set([graphPath]));
  });
});
