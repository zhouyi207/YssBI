import { describe, expect, it } from 'vitest';
import {
  estimatedLogListHeight,
  LOG_ROW_STRIDE,
  snapLogViewportToBottom,
} from './logPanelViewport';

describe('logPanelViewport', () => {
  it('estimates list height from fixed row stride', () => {
    expect(estimatedLogListHeight(0)).toBe(0);
    expect(estimatedLogListHeight(3)).toBe(3 * LOG_ROW_STRIDE);
  });

  it('snaps the native viewport to the tail without smooth scrolling', () => {
    const viewport = {
      clientHeight: 100,
      scrollTop: 0,
      style: { scrollBehavior: '' },
    } as unknown as HTMLElement;

    snapLogViewportToBottom(viewport, 10);
    expect(viewport.scrollTop).toBe(Math.max(0, 10 * LOG_ROW_STRIDE - 100));
    expect(viewport.style.scrollBehavior).toBe('');
  });
});
