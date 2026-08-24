// @vitest-environment happy-dom

import { beforeEach, describe, expect, it } from 'vitest';
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

  it('resolveActivityBarLayout maps the persisted position only', () => {
    expect(resolveActivityBarLayout('Hidden')).toEqual({ visible: false, side: 'left' });
    expect(resolveActivityBarLayout('Left')).toEqual({ visible: true, side: 'left' });
    expect(resolveActivityBarLayout('Right')).toEqual({ visible: true, side: 'right' });
    expect(resolveActivityBarLayout(undefined)).toEqual({ visible: true, side: 'left' });
  });
});
