// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createPersistedWindow } from './createPersistedWindow';
import {
  buildSecondaryEditorWindowRequest,
  openSecondaryEditorWindow,
} from './openSecondaryEditorWindow';

vi.mock('./createPersistedWindow', () => ({
  createPersistedWindow: vi.fn(),
}));

describe('openSecondaryEditorWindow', () => {
  beforeEach(() => {
    vi.mocked(createPersistedWindow).mockReset();
    localStorage.clear();
  });

  it('passes an explicit editor-route request to createPersistedWindow', async () => {
    await openSecondaryEditorWindow();

    expect(createPersistedWindow).toHaveBeenCalledOnce();
    expect(createPersistedWindow).toHaveBeenCalledWith(expect.objectContaining({
      label: expect.stringMatching(/^window-/),
      url: 'index.html#/editor',
      visible: true,
    }));
  });

  it('uses per-label secondary geometry instead of main window geometry', () => {
    localStorage.setItem('yssbi-secondary-window-window-2', JSON.stringify({
      width: 840,
      height: 620,
      x: 120,
      y: 90,
      isMaximized: true,
    }));

    const request = buildSecondaryEditorWindowRequest('window-2');

    expect(request).not.toHaveProperty('kind');
    expect(request).toMatchObject({
      geometry: {
        source: 'provided',
        state: {
          width: 840,
          height: 620,
          x: 120,
          y: 90,
          isMaximized: true,
        },
      },
    });
  });
});
