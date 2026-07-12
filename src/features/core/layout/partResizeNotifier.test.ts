// @vitest-environment happy-dom

import { afterEach, describe, expect, it, vi } from 'vitest';
import { schedulePartResizeCommit, PART_RESIZE_COMMIT_EVENT } from './partResizeNotifier';

describe('partResizeNotifier', () => {
  afterEach(() => {
    document.body.classList.remove('layout-sash-dragging');
    vi.useRealTimers();
  });

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
  });

  it('reschedules a pending commit when a debounce flush occurs during sash dragging', () => {
    vi.useFakeTimers();
    const handler = vi.fn();
    window.addEventListener(PART_RESIZE_COMMIT_EVENT, handler);
    document.body.classList.add('layout-sash-dragging');

    schedulePartResizeCommit('panel', 420);
    vi.advanceTimersByTime(100);
    expect(handler).not.toHaveBeenCalled();

    document.body.classList.remove('layout-sash-dragging');
    vi.advanceTimersByTime(100);
    expect(handler).toHaveBeenCalledTimes(1);
    expect((handler.mock.calls[0][0] as CustomEvent).detail).toEqual({
      partId: 'panel',
      pixelSize: 420,
    });

    window.removeEventListener(PART_RESIZE_COMMIT_EVENT, handler);
  });

  it('preserves a commit scheduled reentrantly by a dispatch listener', () => {
    vi.useFakeTimers();
    const details: Array<{ partId: string; pixelSize: number }> = [];
    const handler = (event: Event) => {
      const detail = (event as CustomEvent).detail;
      details.push(detail);
      if (detail.partId === 'panel') {
        schedulePartResizeCommit('detail', 360);
      }
    };
    window.addEventListener(PART_RESIZE_COMMIT_EVENT, handler);

    schedulePartResizeCommit('panel', 420);
    vi.advanceTimersByTime(100);
    expect(details).toEqual([{ partId: 'panel', pixelSize: 420 }]);

    vi.advanceTimersByTime(100);
    expect(details).toEqual([
      { partId: 'panel', pixelSize: 420 },
      { partId: 'detail', pixelSize: 360 },
    ]);

    window.removeEventListener(PART_RESIZE_COMMIT_EVENT, handler);
  });
});
