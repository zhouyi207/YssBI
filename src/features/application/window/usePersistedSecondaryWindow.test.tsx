// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { usePersistedSecondaryWindow } from './usePersistedSecondaryWindow';

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: vi.fn(),
}));

function Harness(): null {
  usePersistedSecondaryWindow();
  return null;
}

describe('usePersistedSecondaryWindow', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
    localStorage.clear();
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    host.remove();
  });

  it('persists restorable geometry when first closed maximized', async () => {
    let closeListener: (() => Promise<void>) | undefined;
    const unlisten = vi.fn();
    vi.mocked(getCurrentWindow).mockReturnValue({
      label: 'window-2',
      onCloseRequested: vi.fn(async (listener: () => Promise<void>) => {
        closeListener = listener;
        return unlisten;
      }),
      isMaximized: vi.fn(async () => true),
    } as unknown as ReturnType<typeof getCurrentWindow>);

    await act(async () => {
      root.render(<Harness />);
    });
    await act(async () => {
      await closeListener?.();
    });

    expect(JSON.parse(localStorage.getItem('yssbi-secondary-window-window-2') ?? 'null')).toEqual({
      width: 1000,
      height: 700,
      x: expect.any(Number),
      y: expect.any(Number),
      isMaximized: true,
    });
  });

  it('disposes a close listener that resolves after unmount', async () => {
    let resolveListener: ((unlisten: () => void) => void) | undefined;
    const unlisten = vi.fn();
    vi.mocked(getCurrentWindow).mockReturnValue({
      label: 'window-2',
      onCloseRequested: vi.fn(() => new Promise<() => void>((resolve) => {
        resolveListener = resolve;
      })),
    } as unknown as ReturnType<typeof getCurrentWindow>);

    await act(async () => {
      root.render(<Harness />);
    });
    await act(async () => {
      root.unmount();
    });
    resolveListener?.(unlisten);
    await Promise.resolve();

    expect(unlisten).toHaveBeenCalledOnce();
    root = createRoot(host);
  });
});
