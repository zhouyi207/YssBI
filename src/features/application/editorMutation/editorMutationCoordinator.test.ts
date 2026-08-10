import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { buildGraphResourceMeta, resourceKey, useResourceStore } from '@/features/core/resource';
import { projectPublicationCoordinator } from './projectPublicationCoordinator';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import type { GraphMutationResultDto } from '@/shared/types/dto/editorMutation';
import {
  executeEditorMutation,
  resetEditorMutationCoordinator,
} from './editorMutationCoordinator';

const projectInstanceId = '00000000-0000-0000-0000-000000000601';
const replacementId = '00000000-0000-0000-0000-000000000699';
const graphPath = 'functions/Main.yssbi-function';
const operationId = '00000000-0000-0000-0000-000000000602';
const locale = 'en-US';

function graphMutationResult(): GraphMutationResultDto {
  return {
    projectInstanceId,
    delta: {
      graphPath,
      fromRevision: 1,
      toRevision: 2,
      causedBy: operationId,
      payload: { operations: [] },
    },
    projectionReplacement: {
      graphPath,
      projection: makeEditorProjectionFixture({
        graphPath,
        sourceRevision: 2,
        title: 'Committed',
      }).projection,
    },
    history: { canUndo: true, canRedo: false },
  };
}

function deferred<T>(): {
  promise: Promise<T>;
  resolve(value: T): void;
} {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((settle) => {
    resolve = settle;
  });
  return { promise, resolve };
}

describe('executeEditorMutation lifecycle identity', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    resetEditorMutationCoordinator();
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
    useGraphDataStore.setState({ graphEntities: {} });
    useResourceStore.getState().clear();
    useResourceStore.getState().upsertResource(
      buildGraphResourceMeta('function', graphPath, 'Main', { revision: 1 }),
    );
    useGraphDataStore.getState().replaceProjection(
      graphPath,
      makeEditorProjectionFixture({ graphPath, sourceRevision: 1, title: 'Current' }).projection,
      1,
    );
  });

  it('advances sidebar resource revision after installing an authoritative graph mutation', async () => {
    await expect(executeEditorMutation(
      {
        graphPath,
        locale,
        mutation: { type: 'deleteNode', payload: { nodeId: 'local-node' } },
      },
      {
        createOperationId: () => operationId,
        mutateGraph: vi.fn().mockResolvedValue(graphMutationResult()),
        hydrateGraph: vi.fn(),
        updateHistoryStatus: vi.fn(),
      },
    )).resolves.toMatchObject({ status: 'applied' });

    expect(useResourceStore.getState().resources[
      resourceKey({ id: graphPath, kind: 'function' })
    ]?.revision).toBe(2);
  });

  it('passes one captured identity and ignores completion after project replacement', async () => {
    const result = deferred<GraphMutationResultDto>();
    const mutateGraph = vi.fn().mockReturnValue(result.promise);
    const applyStoreEffect = vi.fn();

    const completion = executeEditorMutation(
      {
        graphPath,
        locale,
        mutation: { type: 'deleteNode', payload: { nodeId: 'local-node' } },
      },
      {
        createOperationId: () => operationId,
        mutateGraph,
        hydrateGraph: vi.fn(),
        updateHistoryStatus: applyStoreEffect,
      },
    );

    expect(mutateGraph).toHaveBeenCalledWith(
      projectInstanceId,
      graphPath,
      locale,
      expect.any(Object),
    );
    projectPublicationCoordinator.startProject(replacementId, 0);
    const mutationResult = graphMutationResult();
    result.resolve(mutationResult);

    await expect(completion).resolves.toEqual({ status: 'stale', result: mutationResult });
    expect(applyStoreEffect).not.toHaveBeenCalled();
    expect(useGraphDataStore.getState().graphEntities[graphPath].sourceRevision).toBe(1);
  });

  it('treats a backend stale lifecycle rejection as stale without store effects', async () => {
    const applyStoreEffect = vi.fn();
    const hydrateGraph = vi.fn();

    await expect(executeEditorMutation(
      {
        graphPath,
        locale,
        mutation: { type: 'deleteNode', payload: { nodeId: 'local-node' } },
      },
      {
        createOperationId: () => operationId,
        mutateGraph: vi.fn().mockRejectedValue({
          code: 'stale_project_lifecycle',
          message: 'project was replaced',
        }),
        hydrateGraph,
        updateHistoryStatus: applyStoreEffect,
      },
    )).resolves.toEqual({ status: 'stale' });

    expect(hydrateGraph).not.toHaveBeenCalled();
    expect(applyStoreEffect).not.toHaveBeenCalled();
    expect(useGraphDataStore.getState().graphEntities[graphPath].sourceRevision).toBe(1);
  });

  it('does not invoke the command when replacement occurs while reading revision authority', async () => {
    const capturedStore = useGraphDataStore.getState();
    vi.spyOn(useGraphDataStore, 'getState').mockImplementationOnce(() => {
      projectPublicationCoordinator.startProject(replacementId, 0);
      return capturedStore;
    });
    const mutateGraph = vi.fn().mockResolvedValue(graphMutationResult());

    await expect(executeEditorMutation(
      {
        graphPath,
        locale,
        mutation: { type: 'deleteNode', payload: { nodeId: 'local-node' } },
      },
      {
        createOperationId: () => operationId,
        mutateGraph,
        hydrateGraph: vi.fn(),
        updateHistoryStatus: vi.fn(),
      },
    )).rejects.toMatchObject({ code: 'stale_project_lifecycle' });

    expect(mutateGraph).not.toHaveBeenCalled();
    expect(useGraphDataStore.getState().graphEntities[graphPath].sourceRevision).toBe(1);
  });
});
