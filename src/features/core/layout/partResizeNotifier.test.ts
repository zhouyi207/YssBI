// @vitest-environment happy-dom

import { describe, expect, it, vi } from 'vitest';
import { schedulePartResizeCommit, PART_RESIZE_COMMIT_EVENT } from './partResizeNotifier';

describe('partResizeNotifier', () => {
  it('emits debounced part resize commit events', async () => {
    vi.useFakeTimers();
    const handler = vi.fn();
    window.addEventListener(PART_RESIZE_COMMIT_EVENT, handler);

    schedulePartResizeCommit('panel', 240);
    expect(handler).not.toHaveBeenCalled();

    vi.advanceTimersByTime(100);
    expect(handler).toHaveBeenCalledTimes(1);
    expect((handler.mock.calls[0][0] as CustomEvent).detail).toEqual({
      partId: 'panel',
      pixelSize: 240,
    });

    window.removeEventListener(PART_RESIZE_COMMIT_EVENT, handler);
    vi.useRealTimers();
  });
});
