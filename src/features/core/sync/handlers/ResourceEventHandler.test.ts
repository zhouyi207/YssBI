import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';


import { toGraphResourceUri } from '@/shared/types/domain/graphResourcePath';
import { useDocumentStateStore, useResourceStore } from '@/features/core/resource';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import {
  ProjectIndexInvalidatedHandler,
  ResourceChangedHandler,
} from './ResourceEventHandler';

describe('Resource event handlers', () => {
  beforeEach(() => {
    vi.clearAllMocks();
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



  it('ProjectIndexInvalidatedHandler coalesces bursts into one refreshResourceIndex call', async () => {
    vi.useFakeTimers();
    const refreshResourceIndex = vi.fn().mockResolvedValue(true);
    const baseState = useProjectIOStore.getState();
    vi.spyOn(useProjectIOStore, 'getState').mockReturnValue({
      ...baseState,
      refreshResourceIndex,
    });

    const handler = new ProjectIndexInvalidatedHandler();
    handler.handle({ source: 'watcher', version: 1 });
    handler.handle({ source: 'watcher', version: 2 });
    handler.handle({ source: 'watcher', version: 3 });

    expect(refreshResourceIndex).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(50);
    expect(refreshResourceIndex).toHaveBeenCalledTimes(1);
  });
});
