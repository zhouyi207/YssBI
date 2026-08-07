import { beforeEach, describe, expect, it } from 'vitest';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import type { ResourceMutationResultDto } from '@/shared/types/dto/editorMutation';
import { prepareSynchronousPublicationCommit } from './resourceMutationResult';

const caller = 'events/Caller.yssbi-event';
const oldTarget = 'functions/Old.yssbi-function';

function callerSnapshot() {
  return structuredClone(useGraphDataStore.getState().graphEntities[caller]);
}

describe('resource mutation projection replacement protocol', () => {
  beforeEach(() => {
    useGraphDataStore.setState({ graphEntities: {} });
    const projection = makeEditorProjectionFixture({
      graphPath: caller,
      nodeId: 'call-1',
      nodeTypeId: 'yssbi.project.function.call',
      title: 'Loaded caller',
    }).projection;
    useGraphDataStore.getState().replaceProjection(caller, projection, 1);
    useGraphDataStore.setState((state) => ({
      graphEntities: {
        ...state.graphEntities,
        [caller]: {
          ...state.graphEntities[caller],
          nodes: {
            ...state.graphEntities[caller].nodes,
            'call-1': {
              ...state.graphEntities[caller].nodes['call-1'],
              subGraphPath: oldTarget,
            },
          },
        },
      },
    }));
  });

  it('rejects complete status missing a caller replacement without locally patching the caller', () => {
    const before = callerSnapshot();
    const result: ResourceMutationResultDto = {
      operationId: '00000000-0000-0000-0000-000000000904',
      projectInstanceId: '00000000-0000-0000-0000-000000000901',
      publicationRevision: 1,
      moves: [],
      deltas: [],
      projectionReplacements: [],
      projectionStatus: { status: 'complete', expectedGraphPaths: [caller] },
      history: { canUndo: false, canRedo: false },
    };

    expect(() => prepareSynchronousPublicationCommit(result, {
      projectInstanceId: result.projectInstanceId,
      epoch: 1,
      fingerprint: 'missing-loaded-caller-replacement',
      affectedGraphPaths: new Set([caller]),
      moves: [],
    })).toThrow('complete replacement paths do not equal the declared expected graph paths');
    expect(callerSnapshot()).toEqual(before);
    expect(useGraphDataStore.getState().graphEntities[caller]?.nodes['call-1']?.subGraphPath)
      .toBe(oldTarget);
  });
});
