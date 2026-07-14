import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import {
  commitAfterCommand,
  notifyIndexInvalidated,
  resetResourceIndexCoordinatorForTests,
} from './resourceIndexCoordinator';

describe('resourceIndexCoordinator', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    resetResourceIndexCoordinatorForTests();
  });

  afterEach(() => {
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

    void notifyIndexInvalidated('watcher');
    void notifyIndexInvalidated('watcher');

    await vi.advanceTimersByTimeAsync(50);

    expect(refreshResourceIndex).toHaveBeenCalledTimes(1);
  });

  it('suppresses a watcher echo immediately following a command refresh', async () => {
    const refreshResourceIndex = vi.fn().mockResolvedValue(true);
    vi.spyOn(useProjectIOStore, 'getState').mockReturnValue({
      ...useProjectIOStore.getState(),
      refreshResourceIndex,
    });

    await commitAfterCommand();
    await notifyIndexInvalidated('watcher');
    await vi.advanceTimersByTimeAsync(500);

    expect(refreshResourceIndex).toHaveBeenCalledTimes(1);
  });
});
