import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useGraphMetaStore } from '@/features/core/dataStore';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import { useResourceStore } from '@/features/core/resource';
import { GraphService } from '@/services/graph/graphService';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import { renameResource } from './resourceActions';

vi.mock('@/features/application/editorMutation/projectPublicationCoordinator', () => ({
  projectPublicationCoordinator: {
    submit: vi.fn(async () => ({ status: 'applied', affectedGraphPaths: new Set() })),
  },
}));

function renameResult(projectInstanceId: string, publicationRevision = 1) {
  return {
    projectInstanceId,
    publicationRevision,
    moves: [{
      from: 'events/Old.yssbi-event',
      to: 'events/New.yssbi-event',
      kind: 'event' as const,
      name: 'New',
    }],
    deltas: [{
      resource: { kind: 'graph' as const, key: 'events/New.yssbi-event' },
      fromRevision: 0,
      toRevision: 1,
      causedBy: '00000000-0000-0000-0000-000000000123',
      payload: {
        kind: 'graph_resource_move' as const,
        patch: {
          from: 'events/Old.yssbi-event',
          to: 'events/New.yssbi-event',
        },
      },
    }],
    projectionReplacements: [],
    projectionStatus: {
      status: 'incomplete' as const,
      invalidatedGraphPaths: ['events/New.yssbi-event'],
    },
    history: { canUndo: true, canRedo: false },
  };
}

describe('renameResource project ownership', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.clearAllMocks();
    useResourceStore.getState().clear();
    useGraphMetaStore.getState().clear();
    useProjectIOStore.setState({ projectInstanceId: 'project-instance-current' });
  });

  it('rejects a stale rename receipt before coordinator submission', async () => {
    vi.spyOn(GraphService, 'renameGraphResource').mockResolvedValue(
      renameResult('project-instance-stale'),
    );

    await expect(
      renameResource({ id: 'events/Old.yssbi-event', kind: 'event' }, 'New'),
    ).rejects.toThrow('stale project lifecycle');

    expect(projectPublicationCoordinator.submit).not.toHaveBeenCalled();
  });

  it('delegates the canonical rename receipt without installing the destination independently', async () => {
    const committed = renameResult('project-instance-current');
    vi.spyOn(GraphService, 'renameGraphResource').mockResolvedValue(committed);
    useResourceStore.getState().setSnapshot({
      resources: [{
        id: 'events/Old.yssbi-event',
        kind: 'event',
        name: 'Old',
        uri: 'yssbi://event/events/Old.yssbi-event',
        exists: true,
        loaded: false,
        hasDirtyDocument: false,
        hasStaleDocument: false,
        hasConflictDocument: false,
      }],
      graphOrder: ['events/Old.yssbi-event'],
    });
    useGraphMetaStore.setState({
      graphs: {
        'events/Old.yssbi-event': {
          path: 'events/Old.yssbi-event',
          name: 'Old',
          type: 'event',
        },
      },
    });
    const resourcesBefore = useResourceStore.getState().resources;
    const graphOrderBefore = useResourceStore.getState().graphOrder;
    const graphMetaBefore = useGraphMetaStore.getState().graphs;

    await renameResource({ id: 'events/Old.yssbi-event', kind: 'event' }, 'New');

    expect(GraphService.renameGraphResource).toHaveBeenCalledWith(
      'project-instance-current',
      'events/Old.yssbi-event',
      'New',
    );
    expect(projectPublicationCoordinator.submit).toHaveBeenCalledOnce();
    expect(projectPublicationCoordinator.submit).toHaveBeenCalledWith({ result: committed });
    expect(useResourceStore.getState().resources).toBe(resourcesBefore);
    expect(useResourceStore.getState().graphOrder).toBe(graphOrderBefore);
    expect(useGraphMetaStore.getState().graphs).toBe(graphMetaBefore);
  });

  it('rejects a matching receipt when project ownership changes in flight', async () => {
    let resolveRename!: (value: Awaited<ReturnType<typeof GraphService.renameGraphResource>>) => void;
    vi.spyOn(GraphService, 'renameGraphResource').mockReturnValue(new Promise((resolve) => {
      resolveRename = resolve;
    }));

    const pending = renameResource({ id: 'events/Old.yssbi-event', kind: 'event' }, 'New');
    useProjectIOStore.setState({ projectInstanceId: 'project-instance-replacement' });
    resolveRename(renameResult('project-instance-current'));

    await expect(pending).rejects.toThrow('stale project lifecycle');
    expect(projectPublicationCoordinator.submit).not.toHaveBeenCalled();
  });
});
