import { beforeEach, describe, expect, it, vi } from 'vitest';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { WindowStateService } from '@/services/window/windowStateService';
import {
  createPersistedWindow,
  type PersistedWindowOptions,
} from './createPersistedWindow';

vi.mock('@tauri-apps/api/webviewWindow', () => ({
  WebviewWindow: vi.fn(),
}));

vi.mock('@/services/window/windowStateService', () => ({
  WindowStateService: {
    get: vi.fn(),
  },
}));

describe('createPersistedWindow', () => {
  beforeEach(() => {
    vi.mocked(WebviewWindow).mockClear();
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
    expect(WebviewWindow).toHaveBeenCalledWith('window-2', expect.objectContaining({
      width: 840,
      height: 620,
      x: 120,
      y: 90,
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
    expect(WebviewWindow).toHaveBeenCalledWith('logs-2', expect.objectContaining({
      x: 140,
      y: 110,
    }));
  });
});
