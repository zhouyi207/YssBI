import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useProjectIOStore } from '@/features/application/project/projectIOStore';
import {
  captureProjectIdentity,
  clearProjectLifecycle,
  startProjectLifecycle,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import {
  commitAfterCommand,
  notifyIndexInvalidated,
  resetResourceIndexCoordinatorForTests,
} from './resourceIndexCoordinator';

describe('resourceIndexCoordinator', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    resetResourceIndexCoordinatorForTests();
    startProjectLifecycle('project-instance-a');
  });

  afterEach(() => {
    resetResourceIndexCoordinatorForTests();
    clearProjectLifecycle();
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('refreshes immediately after a successful command', async () => {
    const refreshResourceIndex = vi.fn().mockResolvedValue(true);
    vi.spyOn(useProjectIOStore, 'getState').mockReturnValue({
      ...useProjectIOStore.getState(),
      refreshResourceIndex,
    });

    await commitAfterCommand();

    expect(refreshResourceIndex).toHaveBeenCalledTimes(1);
  });

  it('coalesces repeated external invalidations into one refresh', async () => {
    const refreshResourceIndex = vi.fn().mockResolvedValue(true);
    vi.spyOn(useProjectIOStore, 'getState').mockReturnValue({
      ...useProjectIOStore.getState(),
      refreshResourceIndex,
    });

    const identity = captureProjectIdentity();
    void notifyIndexInvalidated(identity, 1);
    void notifyIndexInvalidated(identity, 2);

    await vi.advanceTimersByTimeAsync(50);

    expect(refreshResourceIndex).toHaveBeenCalledTimes(1);
  });

  it('does not let a project A command refresh suppress a project B watcher invalidation', async () => {
    let resolveCommandRefresh!: (result: boolean) => void;
    const commandRefreshPending = new Promise<boolean>((resolve) => {
      resolveCommandRefresh = resolve;
    });
    const refreshResourceIndex = vi.fn()
      .mockImplementationOnce(() => commandRefreshPending)
      .mockResolvedValue(true);
    vi.spyOn(useProjectIOStore, 'getState').mockReturnValue({
      ...useProjectIOStore.getState(),
      refreshResourceIndex,
    });

    const commandRefresh = commitAfterCommand();
    startProjectLifecycle('project-instance-b');
    const watcherRefresh = notifyIndexInvalidated(captureProjectIdentity(), 1);

    await vi.advanceTimersByTimeAsync(50);

    expect(refreshResourceIndex).toHaveBeenCalledTimes(2);

    resolveCommandRefresh(true);
    await Promise.all([commandRefresh, watcherRefresh]);
  });

  it('refreshes again when a higher watcher version arrives during a pending refresh', async () => {
    let resolveFirstRefresh!: (result: boolean) => void;
    const firstRefreshPending = new Promise<boolean>((resolve) => {
      resolveFirstRefresh = resolve;
    });
    const refreshResourceIndex = vi.fn()
      .mockImplementationOnce(() => firstRefreshPending)
      .mockResolvedValue(true);
    vi.spyOn(useProjectIOStore, 'getState').mockReturnValue({
      ...useProjectIOStore.getState(),
      refreshResourceIndex,
    });
    const identity = captureProjectIdentity();

    const firstInvalidation = notifyIndexInvalidated(identity, 1);
    await vi.advanceTimersByTimeAsync(50);
    const higherInvalidation = notifyIndexInvalidated(identity, 2);
    resolveFirstRefresh(true);

    await Promise.all([firstInvalidation, higherInvalidation]);

    expect(refreshResourceIndex).toHaveBeenCalledTimes(2);
  });
});
