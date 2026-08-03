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

const projectInstanceId = '00000000-0000-0000-0000-000000000911';
const operationId = '00000000-0000-0000-0000-000000000912';
const graphPath = 'events/Created.yssbi-event';

function lifecycleResult(
  publicationRevision: number,
  before: { revision: number; path: string; kind: 'event' | 'function' } | null,
  after: { revision: number; path: string; kind: 'event' | 'function' } | null,
): ResourceMutationResultDto {
  return {
    operationId,
    projectInstanceId,
    publicationRevision,
    moves: [],
    deltas: [{
      resource: { kind: 'graph', key: graphPath },
      fromRevision: 0,
      toRevision: 1,
      causedBy: operationId,
      payload: {
        kind: 'graph_resource_lifecycle',
        patch: { before, after },
      },
    }],
    projectionReplacements: [],
    projectionStatus: { status: 'incomplete', invalidatedGraphPaths: [] },
    history: { canUndo: false, canRedo: false },
  } as unknown as ResourceMutationResultDto;
}

const present = { revision: 0, path: graphPath, kind: 'event' as const };

function resetStores(): void {
  useGraphDataStore.setState({ graphEntities: {} });
  useGraphMetaStore.setState({ graphs: {} });
  useResourceStore.getState().clear();
  useDocumentStateStore.getState().clear();
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

  it('uses lifecycle payload paths when selecting recovery hydration', () => {
    const created = lifecycleResult(2, null, present);
    const paths = collectProjectRecoveryGraphPaths(
      {
        projectInstanceId,
        projectName: 'Current',
        appVersion: '0.2.7',
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
