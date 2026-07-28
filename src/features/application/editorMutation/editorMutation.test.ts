import { invoke } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import {
  FunctionMutationService,
  GraphMutationService,
  HistoryService,
} from '@/services/nodeSystem';
import type {
  EditorGraphMutationDto,
  GraphMutationResultDto,
  MutationRequestDto,
  ResourceMutationResultDto,
} from '@/shared/types/dto/editorMutation';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import {
  executeEditorMutation,
  resetEditorMutationCoordinator,
} from './editorMutationCoordinator';
import {
  getPendingMutation,
  resetPendingMutations,
} from './pendingMutationRegistry';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const graphPath = 'functions/Main.yssbi-function';

function deleteNodeMutation(): EditorGraphMutationDto {
  return { type: 'deleteNode', payload: { nodeId: 'local-node' } };
}

function graphResult(
  operationId: string,
  fromRevision = 1,
  toRevision = 2,
): GraphMutationResultDto {
  return {
    delta: {
      graphPath,
      fromRevision,
      toRevision,
      causedBy: operationId,
      payload: { operations: [] },
    },
    projectionReplacement: {
      graphPath,
      projection: makeEditorProjectionFixture({
        graphPath,
        sourceRevision: toRevision,
        title: `Revision ${toRevision}`,
      }).projection,
    },
    history: { canUndo: true, canRedo: false },
  };
}

describe('mutation and history services', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('sends the canonical declared port mutation JSON', async () => {
    const request: MutationRequestDto<EditorGraphMutationDto> = {
      resource: { kind: 'graph', key: graphPath },
      baseRevision: 4,
      operationId: '00000000-0000-0000-0000-000000000389',
      payload: {
        type: 'setLiteral',
        payload: {
          address: {
            kind: 'declared',
            nodeId: '00000000-0000-0000-0000-000000000385',
            portKey: 'value',
          },
          literal: 42,
        },
      },
    };
    vi.mocked(invoke).mockResolvedValue(graphResult(request.operationId, 4, 5));

    await GraphMutationService.mutateGraph(graphPath, 'en-US', request);

    expect(invoke).toHaveBeenCalledWith('mutate_graph_document', {
      graphPath: 'functions/Main.yssbi-function',
      locale: 'en-US',
      request: {
        resource: { kind: 'graph', key: 'functions/Main.yssbi-function' },
        baseRevision: 4,
        operationId: '00000000-0000-0000-0000-000000000389',
        payload: {
          type: 'setLiteral',
          payload: {
            address: {
              kind: 'declared',
              nodeId: '00000000-0000-0000-0000-000000000385',
              portKey: 'value',
            },
            literal: 42,
          },
        },
      },
    });
  });

  it('sends the canonical dynamic instance mutation JSON', async () => {
    const request: MutationRequestDto<EditorGraphMutationDto> = {
      resource: { kind: 'graph', key: graphPath },
      baseRevision: 5,
      operationId: '00000000-0000-0000-0000-00000000038a',
      payload: {
        type: 'removePortInstance',
        payload: {
          address: {
            kind: 'instance',
            nodeId: '00000000-0000-0000-0000-000000000386',
            templateKey: 'inputs',
            instanceId: '00000000-0000-0000-0000-000000000388',
          },
        },
      },
    };
    vi.mocked(invoke).mockResolvedValue(graphResult(request.operationId, 5, 6));

    await GraphMutationService.mutateGraph(graphPath, 'zh-CN', request);

    expect(invoke).toHaveBeenCalledWith('mutate_graph_document', {
      graphPath: 'functions/Main.yssbi-function',
      locale: 'zh-CN',
      request: {
        resource: { kind: 'graph', key: 'functions/Main.yssbi-function' },
        baseRevision: 5,
        operationId: '00000000-0000-0000-0000-00000000038a',
        payload: {
          type: 'removePortInstance',
          payload: {
            address: {
              kind: 'instance',
              nodeId: '00000000-0000-0000-0000-000000000386',
              templateKey: 'inputs',
              instanceId: '00000000-0000-0000-0000-000000000388',
            },
          },
        },
      },
    });
  });

  it('models canonical resource delta JSON with camelCase correlation fields', () => {
    const result: ResourceMutationResultDto = {
      projectInstanceId: '00000000-0000-0000-0000-000000000601',
      publicationRevision: 3,
      moves: [],
      deltas: [
        {
          resource: { kind: 'graph', key: 'events/Main.yssbi-event' },
          fromRevision: 4,
          toRevision: 5,
          causedBy: '00000000-0000-0000-0000-000000000389',
          payload: {
            kind: 'graph',
            patch: { operations: [] },
          },
        },
      ],
      projectionReplacements: [],
      projectionStatus: {
        status: 'complete',
        expectedGraphPaths: ['events/Main.yssbi-event'],
      },
      history: { canUndo: true, canRedo: false },
    };

    expect(JSON.parse(JSON.stringify(result))).toEqual({
      projectInstanceId: '00000000-0000-0000-0000-000000000601',
      publicationRevision: 3,
      moves: [],
      deltas: [
        {
          resource: { kind: 'graph', key: 'events/Main.yssbi-event' },
          fromRevision: 4,
          toRevision: 5,
          causedBy: '00000000-0000-0000-0000-000000000389',
          payload: {
            kind: 'graph',
            patch: { operations: [] },
          },
        },
      ],
      projectionReplacements: [],
      projectionStatus: {
        status: 'complete',
        expectedGraphPaths: ['events/Main.yssbi-event'],
      },
      history: { canUndo: true, canRedo: false },
    });
  });

  it('sends revisioned function signature requests through the thin node-system service', async () => {
    const request = {
      resource: { kind: 'function' as const, key: graphPath },
      baseRevision: 9,
      operationId: '00000000-0000-0000-0000-000000000502',
      payload: {
        before: { parameters: [], return_type: null },
        after: {
          parameters: [{ id: 'value', name: 'Value', type_name: 'Float64' }],
          return_type: 'Int64',
        },
      },
    };
    const response = {
      projectInstanceId: '00000000-0000-0000-0000-000000000601',
      publicationRevision: 4,
      deltas: [],
      projectionReplacements: [],
      projectionStatus: { status: 'complete' as const, expectedGraphPaths: [] },
      history: { canUndo: true, canRedo: false },
    };
    vi.mocked(invoke).mockResolvedValue(response);

    await expect(
      FunctionMutationService.updateSignature(graphPath, 'zh-CN', request),
    ).resolves.toBe(response);
    expect(invoke).toHaveBeenCalledWith('update_function_signature', {
      functionPath: graphPath,
      locale: 'zh-CN',
      request,
    });
  });

  it('keeps history services invoke-only and sends locale plus request', async () => {
    const request = {
      resource: { kind: 'graph' as const, key: graphPath },
      baseRevision: 5,
      operationId: 'history-operation',
      payload: {},
    };
    const response = {
      projectInstanceId: '00000000-0000-0000-0000-000000000601',
      publicationRevision: 5,
      deltas: [],
      projectionReplacements: [],
      projectionStatus: {
        status: 'complete' as const,
        invalidatedGraphPaths: [graphPath],
      },
      history: { canUndo: false, canRedo: true },
    };
    vi.mocked(invoke).mockResolvedValue(response);

    await expect(HistoryService.undo('zh-CN', request)).resolves.toBe(response);
    expect(invoke).toHaveBeenLastCalledWith('undo_graph_document', {
      locale: 'zh-CN',
      request,
    });

    await HistoryService.redo('zh-CN', request);
    expect(invoke).toHaveBeenLastCalledWith('redo_graph_document', {
      locale: 'zh-CN',
      request,
    });

    await HistoryService.getStatus();
    expect(invoke).toHaveBeenLastCalledWith('get_project_history_status');
  });
});

describe('executeEditorMutation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetPendingMutations();
    resetEditorMutationCoordinator();
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
        mutateGraph: async (_path, _locale, request) => {
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
      nodes: { 'local-node': { title: 'Revision 2' } },
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
