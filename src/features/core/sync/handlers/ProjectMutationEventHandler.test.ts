import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  registerPendingMutation,
  resetPendingMutations,
} from '@/features/application/editorMutation/pendingMutationRegistry';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import { invalidateGraphProjection } from '@/features/application/editorProjection/graphProjectionCoordinator';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { useHistoryStore } from '@/features/core/history';
import { ProjectService } from '@/services/project/projectService';
import { GraphProjectionService } from '@/services/nodeSystem/graphProjectionService';
import type { ResourceMutationResultDto } from '@/shared/types/dto/editorMutation';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import { logger } from '@/utils/appLogger';
import {
  GraphDeltaHandler,
  ResourceMutationCommittedHandler,
} from './ProjectMutationEventHandler';

vi.mock('@/features/application/editorProjection/graphProjectionCoordinator', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/features/application/editorProjection/graphProjectionCoordinator')>()),
  invalidateGraphProjection: vi.fn(async () => true),
}));

vi.mock('@/services/project/projectService', () => ({
  ProjectService: {
    getProjectIndex: vi.fn(),
  },
}));

vi.mock('@/services/nodeSystem/graphProjectionService', () => ({
  GraphProjectionService: {
    loadGraph: vi.fn(),
    hydrateGraph: vi.fn(),
  },
}));

const graphPath = 'events/Main.yssbi-event';
const projectInstanceId = '00000000-0000-0000-0000-000000000601';
const operationId = '00000000-0000-0000-0000-000000000401';

function resourceResult(publicationRevision = 1): ResourceMutationResultDto {
  return {
    projectInstanceId,
    publicationRevision,
    moves: [],
    deltas: [{
      resource: { kind: 'graph', key: graphPath },
      fromRevision: 1,
      toRevision: 2,
      causedBy: operationId,
      payload: { kind: 'graph', patch: { operations: [] } },
    }],
    projectionReplacements: [{
      graphPath,
      projection: makeEditorProjectionFixture({
        graphPath,
        sourceRevision: 2,
        title: 'Committed',
      }).projection,
    }],
    projectionStatus: { status: 'complete', expectedGraphPaths: [graphPath] },
    history: { canUndo: true, canRedo: false },
  };
}

function emptyResult(
  publicationRevision: number,
  history = { canUndo: true, canRedo: false },
): ResourceMutationResultDto {
  return {
    projectInstanceId,
    publicationRevision,
    moves: [],
    deltas: [],
    projectionReplacements: [],
    projectionStatus: { status: 'complete', expectedGraphPaths: [] },
    history,
  };
}

function recoveryIndex(publicationRevision: number) {
  return {
    projectInstanceId,
    publicationRevision,
    history: { canUndo: false, canRedo: false },
    projectName: 'Test',
    graphs: [],
    variables: [],
    worksheets: [],
    exportTime: '',
    appVersion: '0.2.7',
  };
}

describe('Project mutation event synchronization', () => {
  beforeEach(() => {
    projectPublicationCoordinator.cancelProject();
    vi.restoreAllMocks();
    vi.clearAllMocks();
    resetPendingMutations();
    useGraphDataStore.setState({ graphEntities: {} });
    useHistoryStore.setState({ canUndo: false, canRedo: false, pending: false }, true);
  });

  it('suppresses only the GraphDelta echo whose operation ID is pending', () => {
    registerPendingMutation({ operationId, graphPath, baseRevision: 1 });

    new GraphDeltaHandler().handle({
      delta: {
        graphPath,
        fromRevision: 1,
        toRevision: 2,
        causedBy: operationId,
        payload: { operations: [] },
      },
    });

    expect(invalidateGraphProjection).not.toHaveBeenCalled();
  });

  it('requests projection invalidation for a newer external GraphDelta', () => {
    useGraphDataStore.getState().replaceProjection(
      graphPath,
      makeEditorProjectionFixture({ graphPath, sourceRevision: 1 }).projection,
      1,
    );

    new GraphDeltaHandler().handle({
      delta: {
        graphPath,
        fromRevision: 1,
        toRevision: 2,
        causedBy: null,
        payload: { operations: [] },
      },
    });

    expect(invalidateGraphProjection).toHaveBeenCalledOnce();
    expect(invalidateGraphProjection).toHaveBeenCalledWith(graphPath);
  });

  it('ignores GraphDelta revisions already represented by the projection', () => {
    useGraphDataStore.getState().replaceProjection(
      graphPath,
      makeEditorProjectionFixture({ graphPath, sourceRevision: 2 }).projection,
      1,
    );

    new GraphDeltaHandler().handle({
      delta: {
        graphPath,
        fromRevision: 1,
        toRevision: 2,
        causedBy: null,
        payload: { operations: [] },
      },
    });

    expect(invalidateGraphProjection).not.toHaveBeenCalled();
  });

  it('delivers a committed resource result even when its operation ID is pending', () => {
    const submit = vi.spyOn(projectPublicationCoordinator, 'submit').mockResolvedValue({
      status: 'applied',
      affectedGraphPaths: new Set(),
    });
    const result = resourceResult();
    registerPendingMutation({ operationId, graphPath, baseRevision: 1 });

    new ResourceMutationCommittedHandler().handle({ result });

    expect(submit).toHaveBeenCalledOnce();
    expect(submit).toHaveBeenCalledWith({ result });
  });

  it('delivers matching direct and event receipts to coordinator-owned deduplication', () => {
    const submit = vi.spyOn(projectPublicationCoordinator, 'submit').mockResolvedValue({
      status: 'applied',
      affectedGraphPaths: new Set(),
    });
    const result = resourceResult();
    const handler = new ResourceMutationCommittedHandler();

    handler.handle({ result });
    handler.handle({ result: structuredClone(result) });

    expect(submit).toHaveBeenCalledTimes(2);
  });

  it('ignores malformed event envelopes without coordinator submission', () => {
    const submit = vi.spyOn(projectPublicationCoordinator, 'submit').mockResolvedValue({
      status: 'applied',
      affectedGraphPaths: new Set(),
    });
    const handler = new ResourceMutationCommittedHandler();

    handler.handle({ result: null as never });
    handler.handle({ result: 'bad' as never });

    expect(submit).not.toHaveBeenCalled();
  });

  it('logs asynchronous coordinator rejection at the event boundary', async () => {
    const logError = vi.spyOn(logger.sys, 'error').mockImplementation(() => undefined);
    const submit = vi.spyOn(projectPublicationCoordinator, 'submit').mockRejectedValueOnce(
      new Error('publication failed'),
    );

    new ResourceMutationCommittedHandler().handle({ result: resourceResult() });
    await Promise.resolve();
    await Promise.resolve();

    expect(submit).toHaveBeenCalledOnce();
    expect(logError).toHaveBeenCalledWith(
      'Resource publication event failed: publication failed',
      'ResourceMutationCommittedHandler',
    );
  });

  it.each(['event-first', 'direct-first'] as const)(
    '%s matching deliveries settle through one coordinator commit',
    async (order) => {
      projectPublicationCoordinator.startProject(projectInstanceId, 0);
      const result = emptyResult(1);
      const submissions: Promise<unknown>[] = [];
      const originalSubmit = projectPublicationCoordinator.submit.bind(projectPublicationCoordinator);
      const submit = vi.spyOn(projectPublicationCoordinator, 'submit').mockImplementation((input) => {
        const promise = originalSubmit(input);
        submissions.push(promise);
        return promise;
      });
      const setHistory = vi.spyOn(useHistoryStore, 'setState');
      const handler = new ResourceMutationCommittedHandler();

      if (order === 'event-first') {
        handler.handle({ result: structuredClone(result) });
        projectPublicationCoordinator.submit({ result });
      } else {
        projectPublicationCoordinator.submit({ result });
        handler.handle({ result: structuredClone(result) });
      }

      await expect(Promise.all(submissions)).resolves.toMatchObject([
        { status: 'applied' },
        { status: 'duplicate' },
      ]);
      expect(submit).toHaveBeenCalledTimes(2);
      expect(setHistory).toHaveBeenCalledOnce();
      expect(useHistoryStore.getState()).toEqual({
        canUndo: true,
        canRedo: false,
        pending: false,
      });
    },
  );

  it('keeps reverse event arrival revision ordered through the real handler', async () => {
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
    let resolveSnapshot!: (value: ReturnType<typeof recoveryIndex>) => void;
    vi.mocked(ProjectService.getProjectIndex).mockReturnValue(new Promise((resolve) => {
      resolveSnapshot = resolve;
    }));
    const setHistory = vi.spyOn(useHistoryStore, 'setState');
    const handler = new ResourceMutationCommittedHandler();

    handler.handle({ result: emptyResult(2, { canUndo: false, canRedo: true }) });
    await vi.waitFor(() => expect(ProjectService.getProjectIndex).toHaveBeenCalledOnce());
    handler.handle({ result: emptyResult(1, { canUndo: true, canRedo: false }) });
    resolveSnapshot(recoveryIndex(0));

    await vi.waitFor(() => {
      expect(projectPublicationCoordinator.getSnapshotForTests().appliedRevision).toBe(2);
    });
    expect(setHistory.mock.calls.map(([update]) => update)).toEqual([
      { canUndo: true, canRedo: false },
      { canUndo: false, canRedo: true },
    ]);
  });

  it('does not perform fallback graph hydration after coordinator recovery failure', async () => {
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
    vi.spyOn(logger.sys, 'error').mockImplementation(() => undefined);
    vi.mocked(ProjectService.getProjectIndex).mockRejectedValue(new Error('offline'));

    new ResourceMutationCommittedHandler().handle({ result: emptyResult(2) });

    await vi.waitFor(() => {
      expect(projectPublicationCoordinator.getSnapshotForTests()).toMatchObject({
        phase: 'idle',
        pendingRevisions: [],
        appliedRevision: 0,
      });
    });
    expect(GraphProjectionService.loadGraph).not.toHaveBeenCalled();
    expect(GraphProjectionService.hydrateGraph).not.toHaveBeenCalled();
    expect(invalidateGraphProjection).not.toHaveBeenCalled();
  });
});
