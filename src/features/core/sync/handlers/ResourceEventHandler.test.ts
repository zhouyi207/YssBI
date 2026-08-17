import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';



import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import { ProjectIndexInvalidatedHandler } from './ResourceEventHandler';

describe('Resource event handlers', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    projectPublicationCoordinator.cancelProject();
    projectPublicationCoordinator.startProject('project-instance-current', 0);
    useProjectIOStore.setState({ projectInstanceId: 'project-instance-current' });
    vi.restoreAllMocks();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('ignores a stale-project ProjectIndexInvalidated payload', async () => {
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
    await vi.advanceTimersByTimeAsync(50);

    expect(refreshResourceIndex).not.toHaveBeenCalled();
  });

  it('rejects a non-exact ProjectIndexInvalidated payload before refreshing', async () => {
    vi.useFakeTimers();
    const refreshResourceIndex = vi.fn().mockResolvedValue(true);
    const baseState = useProjectIOStore.getState();
    vi.spyOn(useProjectIOStore, 'getState').mockReturnValue({
      ...baseState,
      refreshResourceIndex,
    });

    new ProjectIndexInvalidatedHandler().handle({
      projectInstanceId: 'project-instance-current',
      source: 'watcher',
      version: 1,
      extra: true,
    } as never);
    await vi.advanceTimersByTimeAsync(50);

    expect(refreshResourceIndex).not.toHaveBeenCalled();
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
