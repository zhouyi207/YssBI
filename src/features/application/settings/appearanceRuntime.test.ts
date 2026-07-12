// @vitest-environment happy-dom

import { describe, expect, it, beforeEach } from 'vitest';
import {
  applySmoothScrollSetting,
  resolveActivityBarLayout,
} from './appearanceRuntime';

describe('appearanceRuntime', () => {
  beforeEach(() => {
    delete document.documentElement.dataset.smoothScroll;
  });

  it('applySmoothScrollSetting toggles html data attribute', () => {
    applySmoothScrollSetting(true);
    expect(document.documentElement.dataset.smoothScroll).toBe('true');
    applySmoothScrollSetting(false);
    expect(document.documentElement.dataset.smoothScroll).toBe('false');
  });

  it('resolveActivityBarLayout hides the bar in Zen mode', () => {
    expect(resolveActivityBarLayout('Left', true)).toEqual({ visible: false, side: 'left' });
    expect(resolveActivityBarLayout('Right', true)).toEqual({ visible: false, side: 'left' });
  });

  it('resolveActivityBarLayout maps position settings', () => {
    expect(resolveActivityBarLayout('Hidden', false)).toEqual({ visible: false, side: 'left' });
    expect(resolveActivityBarLayout('Left', false)).toEqual({ visible: true, side: 'left' });
    expect(resolveActivityBarLayout('Right', false)).toEqual({ visible: true, side: 'right' });
  });
});
