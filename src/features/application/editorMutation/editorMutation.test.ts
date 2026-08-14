import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import type {
  EditorGraphMutationDto,
  GraphMutationResultDto,
} from '@/shared/types/dto/editorMutation';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import {
  executeEditorMutation,
  resetEditorMutationCoordinator,
} from './editorMutationCoordinator';
import { projectPublicationCoordinator } from './projectPublicationCoordinator';
import {
  getPendingMutation,
  resetPendingMutations,
} from './pendingMutationRegistry';

const graphPath = 'functions/Main.yssbi-function';
const projectedNodeId = '00000000-0000-0000-0000-000000000603';

function deleteNodeMutation(): EditorGraphMutationDto {
  return { type: 'deleteNodes', payload: { nodeIds: ['local-node'] } };
}

function graphResult(
  operationId: string,
  fromRevision = 1,
  toRevision = 2,
): GraphMutationResultDto {
  return {
    projectInstanceId: '00000000-0000-0000-0000-000000000601',
    delta: {
      graphPath,
      fromRevision,
      toRevision,
      causedBy: operationId,
      payload: {
        operations: [{
          operation: 'remove_node',
          node: {
            id: '00000000-0000-0000-0000-000000000604',
            node_type: 'tests.node',
            position: { x: 0, y: 0 },
            parameters: {},
            user_label: null,
          },
        }],
      },
    },
    projectionReplacement: {
      graphPath,
      projection: makeEditorProjectionFixture({
        graphPath,
        sourceRevision: toRevision,
        nodeId: projectedNodeId,
        title: `Revision ${toRevision}`,
      }).projection,
      functionEditorProjection: {
        functionRevision: toRevision,
        inputs: [],
        outputs: [],
      },
    },
    history: { canUndo: true, canRedo: false },
  };
}

describe('executeEditorMutation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetPendingMutations();
    resetEditorMutationCoordinator();
    projectPublicationCoordinator.startProject(
      '00000000-0000-0000-0000-000000000601',
      0,
    );
    useGraphDataStore.setState({ graphEntities: {} });
    useGraphDataStore.getState().replaceProjection(
      graphPath,
      makeEditorProjectionFixture({ graphPath, sourceRevision: 1, title: 'Current' }).projection,
      1,
    );
  });

  it('registers pending correlation before invoking and applies a correlated committed result', async () => {
    const updateHistoryStatus = vi.fn();
    let pendingObservedDuringInvoke = false;

    const outcome = await executeEditorMutation(
      { graphPath, locale: 'en-US', mutation: deleteNodeMutation() },
      {
        createOperationId: () => 'operation-1',
        mutateGraph: async (_projectInstanceId, _path, _locale, request) => {
          pendingObservedDuringInvoke = getPendingMutation(request.operationId) != null;
          return graphResult(request.operationId);
        },
        hydrateGraph: vi.fn(),
        updateHistoryStatus,
      },
    );

    expect(pendingObservedDuringInvoke).toBe(true);
    expect(outcome.status).toBe('applied');
    expect(useGraphDataStore.getState().graphEntities[graphPath]).toMatchObject({
      sourceRevision: 2,
      nodes: { [projectedNodeId]: { title: 'Revision 2' } },
    });
    expect(updateHistoryStatus).toHaveBeenCalledWith({ canUndo: true, canRedo: false });
    expect(getPendingMutation('operation-1')).toBeUndefined();
  });

  it.each([
    ['operation correlation', graphResult('wrong-operation')],
    ['from revision', graphResult('operation-1', 0, 2)],
    ['monotonic revision', graphResult('operation-1', 1, 3)],
  ])('rejects an invalid committed result %s', async (_case, response) => {
    const hydrateGraph = vi.fn().mockResolvedValue(true);

    await expect(executeEditorMutation(
      { graphPath, locale: 'en-US', mutation: deleteNodeMutation() },
      {
        createOperationId: () => 'operation-1',
        mutateGraph: vi.fn().mockResolvedValue(response),
        hydrateGraph,
        updateHistoryStatus: vi.fn(),
      },
    )).rejects.toThrow(/mutation result/i);

    expect(useGraphDataStore.getState().graphEntities[graphPath].sourceRevision).toBe(1);
    expect(hydrateGraph).toHaveBeenCalledWith(graphPath, 'en-US');
    expect(getPendingMutation('operation-1')).toBeUndefined();
  });

  it('does not let a stale mutation response replace a newer projection', async () => {
    const hydrateGraph = vi.fn().mockResolvedValue(true);
    const newer = makeEditorProjectionFixture({
      graphPath,
      sourceRevision: 3,
      title: 'Newer projection',
    });
    const mutateGraph = vi.fn().mockImplementation(async () => {
      useGraphDataStore.getState().replaceProjection(graphPath, newer.projection, 2);
      return graphResult('operation-1', 1, 2);
    });

    const outcome = await executeEditorMutation(
      { graphPath, locale: 'en-US', mutation: deleteNodeMutation() },
      {
        createOperationId: () => 'operation-1',
        mutateGraph,
        hydrateGraph,
        updateHistoryStatus: vi.fn(),
      },
    );

    expect(outcome.status).toBe('stale');
    expect(useGraphDataStore.getState().graphEntities[graphPath]).toMatchObject({
      sourceRevision: 3,
      nodes: { 'local-node': { title: 'Newer projection' } },
    });
    expect(hydrateGraph).toHaveBeenCalledWith(graphPath, 'en-US');
  });

  it('clears pending state and requests authoritative hydration on revision conflict', async () => {
    const hydrateGraph = vi.fn().mockResolvedValue(true);

    const outcome = await executeEditorMutation(
      { graphPath, locale: 'en-US', mutation: deleteNodeMutation() },
      {
        createOperationId: () => 'operation-conflict',
        mutateGraph: vi.fn().mockRejectedValue({
          code: 'graph_revision_conflict',
          message: 'revision conflict',
        }),
        hydrateGraph,
        updateHistoryStatus: vi.fn(),
      },
    );

    expect(outcome.status).toBe('conflict');
    expect(hydrateGraph).toHaveBeenCalledWith(graphPath, 'en-US');
    expect(getPendingMutation('operation-conflict')).toBeUndefined();
  });
});
