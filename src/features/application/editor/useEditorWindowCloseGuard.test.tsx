// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useEditorWindowCloseGuard } from './useEditorWindowCloseGuard';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

type CloseListener = (event: { preventDefault(): void }) => void | Promise<void>;

const mocks = vi.hoisted(() => ({
  dirty: [] as Array<{ title: string }>,
  confirm3: vi.fn(async () => 'discard' as const),
  saveAllDirtyGraphs: vi.fn(async () => true),
  logError: vi.fn(),
  logWarn: vi.fn(),
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: vi.fn(),
}));
vi.mock('@/features/core/layout/tabDirty', () => ({
  collectDirtyGraphTabs: () => mocks.dirty,
}));
vi.mock('@/features/core/ui/UIStore', () => ({
  uiStore: { confirm3: mocks.confirm3 },
}));
vi.mock('./saveAllDirtyGraphs', () => ({
  saveAllDirtyGraphs: mocks.saveAllDirtyGraphs,
}));
vi.mock('@/app/i18n', () => ({
  i18n: {
    t: (key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? key,
  },
}));
vi.mock('@/utils/appLogger', () => ({
  logger: { app: { error: mocks.logError, warn: mocks.logWarn } },
}));

function Harness(): null {
  useEditorWindowCloseGuard();
  return null;
}

describe('useEditorWindowCloseGuard', () => {
  let host: HTMLDivElement;
  let root: Root | null;

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.dirty = [{ title: 'Main event' }];
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(async () => {
    if (root) {
      await act(async () => root?.unmount());
    }
    host.remove();
  });

  it('guards dirty documents and lets the confirmed close request pass once', async () => {
    let closeListener: CloseListener | undefined;
    let recursivePreventDefault: (() => void) | undefined;
    const unlisten = vi.fn();
    const close = vi.fn(async () => {
      recursivePreventDefault = vi.fn();
      await closeListener?.({ preventDefault: recursivePreventDefault });
    });
    const onCloseRequested = vi.fn(async (listener: CloseListener) => {
      closeListener = listener;
      return unlisten;
    });
    vi.mocked(getCurrentWindow).mockReturnValue({
      close,
      onCloseRequested,
    } as unknown as ReturnType<typeof getCurrentWindow>);

    await act(async () => {
      root?.render(<Harness />);
      await Promise.resolve();
    });

    const preventDefault = vi.fn();
    await act(async () => {
      await closeListener?.({ preventDefault });
    });

    expect(preventDefault).toHaveBeenCalledOnce();
    expect(mocks.confirm3).toHaveBeenCalledOnce();
    expect(mocks.saveAllDirtyGraphs).not.toHaveBeenCalled();
    expect(close).toHaveBeenCalledOnce();
    expect(recursivePreventDefault).not.toHaveBeenCalled();

    await act(async () => root?.unmount());
    root = null;
    expect(unlisten).toHaveBeenCalledOnce();
  });
});
