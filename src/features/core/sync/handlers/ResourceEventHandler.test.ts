import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';


import { toGraphResourceUri } from '@/shared/types/domain/graphResourcePath';
import { useDocumentStateStore, useResourceStore } from '@/features/core/resource';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import {
  ProjectIndexInvalidatedHandler,
  ResourceChangedHandler,
} from './ResourceEventHandler';

describe('Resource event handlers', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    projectPublicationCoordinator.cancelProject();
    projectPublicationCoordinator.startProject('project-instance-current', 0);
    useProjectIOStore.setState({ projectInstanceId: 'project-instance-current' });
    useResourceStore.getState().clear();
    useDocumentStateStore.getState().clear();
    vi.restoreAllMocks();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('ResourceChangedHandler upserts command-origin graph resources into ResourceStore', () => {
    new ResourceChangedHandler().handle({
      projectInstanceId: 'project-instance-current',
      id: 'event-1',
      kind: 'event',
      source: 'command',
      data: {
        id: 'event-1',
        kind: 'event',
        name: 'Renamed Event',
        uri: toGraphResourceUri('event', 'event-1'),
        exists: true,
        loaded: false,
        hasDirtyDocument: false,
        hasStaleDocument: false,
        hasConflictDocument: false,
      },
    });

    expect(useResourceStore.getState().resources[toGraphResourceUri('event', 'event-1')]).toMatchObject({
      id: 'event-1',
      name: 'Renamed Event',
      kind: 'event',
    });
  });



  it('ignores stale resource events before index invalidation or path migration', async () => {
    vi.useFakeTimers();
    const refreshResourceIndex = vi.fn().mockResolvedValue(true);
    const baseState = useProjectIOStore.getState();
    vi.spyOn(useProjectIOStore, 'getState').mockReturnValue({
      ...baseState,
      refreshResourceIndex,
    });

    new ProjectIndexInvalidatedHandler().handle({
      projectInstanceId: 'project-instance-stale',
      source: 'watcher',
      version: 1,
    } as never);
    new ResourceChangedHandler().handle({
      projectInstanceId: 'project-instance-stale',
      id: 'events/Stale.yssbi-event',
      kind: 'event',
      source: 'watcher',
      data: {
        id: 'events/Stale.yssbi-event',
        kind: 'event',
        name: 'Stale',
        uri: toGraphResourceUri('event', 'events/Stale.yssbi-event'),
        exists: true,
        loaded: false,
        hasDirtyDocument: false,
        hasStaleDocument: false,
        hasConflictDocument: false,
      },
    } as never);
    await vi.advanceTimersByTimeAsync(50);

    expect(refreshResourceIndex).not.toHaveBeenCalled();
    expect(useResourceStore.getState().resources).toEqual({});
  });

  it('ProjectIndexInvalidatedHandler coalesces bursts into one refreshResourceIndex call', async () => {
    vi.useFakeTimers();
    const refreshResourceIndex = vi.fn().mockResolvedValue(true);
    const baseState = useProjectIOStore.getState();
    vi.spyOn(useProjectIOStore, 'getState').mockReturnValue({
      ...baseState,
      refreshResourceIndex,
    });

    const handler = new ProjectIndexInvalidatedHandler();
    handler.handle({ projectInstanceId: 'project-instance-current', source: 'watcher', version: 1 });
    handler.handle({ projectInstanceId: 'project-instance-current', source: 'watcher', version: 2 });
    handler.handle({ projectInstanceId: 'project-instance-current', source: 'watcher', version: 3 });

    expect(refreshResourceIndex).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(50);
    expect(refreshResourceIndex).toHaveBeenCalledTimes(1);
  });
});
