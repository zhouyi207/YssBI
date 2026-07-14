import { describe, expect, it } from 'vitest';
import {
  isLogViewportPinnedToBottom,
  shouldLoadOlderLogs,
} from './logPanelScroll';

describe('logPanelScroll', () => {
  it('detects pinned-to-bottom within threshold', () => {
    expect(isLogViewportPinnedToBottom(920, 1000, 100, 80)).toBe(true);
    expect(isLogViewportPinnedToBottom(800, 1000, 100, 80)).toBe(false);
  });

  it('requests older logs near the top edge', () => {
    expect(shouldLoadOlderLogs(100)).toBe(true);
    expect(shouldLoadOlderLogs(200)).toBe(false);
  });
});
