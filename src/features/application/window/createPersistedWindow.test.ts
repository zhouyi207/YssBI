import { beforeEach, describe, expect, it, vi } from 'vitest';
import { WindowStateService } from '@/services/window/windowStateService';
import { createWebviewWindow } from '@/services/platform/webviewWindow';
import {
  createPersistedWindow,
  type PersistedWindowOptions,
} from './createPersistedWindow';

vi.mock('@/services/platform/webviewWindow', () => ({
  createWebviewWindow: vi.fn(),
}));

vi.mock('@/services/window/windowStateService', () => ({
  WindowStateService: {
    get: vi.fn(),
  },
}));

describe('createPersistedWindow', () => {
  beforeEach(() => {
    vi.mocked(createWebviewWindow).mockReset();
    vi.mocked(createWebviewWindow).mockResolvedValue({ ok: true, value: undefined });
    vi.mocked(WindowStateService.get).mockReset();
  });

  it('creates from provided geometry without reading backend state', async () => {
    const request = {
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
      label: 'window-2',
      url: 'index.html#/editor',
      title: 'YssBI Node Editor',
      visible: true,
    } as PersistedWindowOptions;

    await createPersistedWindow(request);

    expect(WindowStateService.get).not.toHaveBeenCalled();
    expect(createWebviewWindow).toHaveBeenCalledWith(expect.objectContaining({
      label: 'window-2',
      url: 'index.html#/editor',
      title: 'YssBI Node Editor',
      width: 840,
      height: 620,
      x: 120,
      y: 90,
      visible: true,
      maximized: true,
    }));
  });

  it('uses backend fallback coordinates only when persisted coordinates are absent', async () => {
    vi.mocked(WindowStateService.get).mockResolvedValue({
      width: 900,
      height: 650,
      x: null,
      y: null,
      isMaximized: false,
    });

    await createPersistedWindow({
      geometry: {
        source: 'backend',
        kind: 'logs',
        fallbackX: 140,
        fallbackY: 110,
      },
      label: 'logs-2',
      url: 'index.html#/logs',
      title: 'Logs',
    });

    expect(WindowStateService.get).toHaveBeenCalledWith('logs');
    expect(createWebviewWindow).toHaveBeenCalledWith(expect.objectContaining({
      label: 'logs-2',
      url: 'index.html#/logs',
      title: 'Logs',
      x: 140,
      y: 110,
    }));
  });

  it('exposes only the stable platform failure code to callers', async () => {
    vi.mocked(createWebviewWindow).mockResolvedValueOnce({
      ok: false,
      failure: { operation: 'createWebviewWindow', code: 'operationFailed' },
    });

    await expect(createPersistedWindow({
      geometry: {
        source: 'provided',
        state: { width: 840, height: 620, x: null, y: null, isMaximized: false },
      },
      label: 'window-3',
      url: 'index.html#/editor',
      title: 'YssBI Node Editor',
    })).rejects.toThrow('operationFailed');
  });
});
