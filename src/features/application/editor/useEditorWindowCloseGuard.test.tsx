// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { WorkbenchLayoutError } from '@/features/core/dockview/workbenchDockviewPort';
import { useEditorWindowCloseGuard } from './useEditorWindowCloseGuard';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

type CloseListener = (event: { preventDefault(): void }) => void | Promise<void>;

type Deferred<T> = {
  readonly promise: Promise<T>;
  readonly resolve: (value: T) => void;
};

function createDeferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((next) => { resolve = next; });
  return { promise, resolve };
}

const mocks = vi.hoisted(() => ({
  dirty: [] as Array<{ title: string }>,
  confirm3: vi.fn(async (): Promise<'confirm' | 'discard' | 'cancel'> => 'discard'),
  saveAllDirtyGraphs: vi.fn(async () => true),
  flushBeforeWindowClose: vi.fn(async (): Promise<void> => undefined),
  showWorkbenchLayoutError: vi.fn(),
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
vi.mock('@/features/application/layout/workbenchLayoutController', () => ({
  workbenchLayoutController: {
    flushBeforeWindowClose: mocks.flushBeforeWindowClose,
  },
}));
vi.mock('@/features/application/layout/workbenchLayoutErrorFeedback', () => ({
  showWorkbenchLayoutError: mocks.showWorkbenchLayoutError,
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

function installWindowMock() {
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

  return {
    close,
    onCloseRequested,
    unlisten,
    getCloseListener(): CloseListener {
      if (!closeListener) throw new Error('close listener is not attached');
      return closeListener;
    },
    getRecursivePreventDefault: () => recursivePreventDefault,
  };
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
    const appWindow = installWindowMock();

    await act(async () => {
      root?.render(<Harness />);
      await Promise.resolve();
    });

    const preventDefault = vi.fn();
    await act(async () => {
      await appWindow.getCloseListener()({ preventDefault });
    });

    expect(preventDefault).toHaveBeenCalledOnce();
    expect(mocks.confirm3).toHaveBeenCalledOnce();
    expect(mocks.saveAllDirtyGraphs).not.toHaveBeenCalled();
    expect(mocks.flushBeforeWindowClose).toHaveBeenCalledOnce();
    expect(appWindow.close).toHaveBeenCalledOnce();
    expect(appWindow.getRecursivePreventDefault()).not.toHaveBeenCalled();

    await act(async () => root?.unmount());
    root = null;
    expect(appWindow.unlisten).toHaveBeenCalledOnce();
  });

  it('waits for deferred hydration and flush before closing a clean window once', async () => {
    const hydrationAndFlush = createDeferred<void>();
    mocks.dirty = [];
    mocks.flushBeforeWindowClose.mockReturnValueOnce(hydrationAndFlush.promise);
    const appWindow = installWindowMock();

    await act(async () => {
      root?.render(<Harness />);
      await Promise.resolve();
    });

    const preventDefault = vi.fn();
    const closeRequest = Promise.resolve(
      appWindow.getCloseListener()({ preventDefault }),
    );
    await Promise.resolve();

    expect(preventDefault).toHaveBeenCalledOnce();
    expect(mocks.confirm3).not.toHaveBeenCalled();
    expect(mocks.flushBeforeWindowClose).toHaveBeenCalledOnce();
    expect(appWindow.close).not.toHaveBeenCalled();

    hydrationAndFlush.resolve(undefined);
    await closeRequest;

    expect(appWindow.close).toHaveBeenCalledOnce();
    expect(appWindow.getRecursivePreventDefault()).not.toHaveBeenCalled();
    expect(mocks.flushBeforeWindowClose).toHaveBeenCalledOnce();
  });

  it('shows typed feedback and leaves the window open when close-time flush fails', async () => {
    const failure = new WorkbenchLayoutError('layout_restore_failed');
    mocks.dirty = [];
    mocks.flushBeforeWindowClose.mockRejectedValueOnce(failure);
    const appWindow = installWindowMock();

    await act(async () => {
      root?.render(<Harness />);
      await Promise.resolve();
    });

    const preventDefault = vi.fn();
    await appWindow.getCloseListener()({ preventDefault });

    expect(preventDefault).toHaveBeenCalledOnce();
    expect(mocks.showWorkbenchLayoutError).toHaveBeenCalledOnce();
    expect(mocks.showWorkbenchLayoutError).toHaveBeenCalledWith(failure);
    expect(appWindow.close).not.toHaveBeenCalled();
  });

  it('coalesces native close requests while the first flush is in flight', async () => {
    const firstFlush = createDeferred<void>();
    mocks.confirm3.mockResolvedValue('confirm');
    mocks.flushBeforeWindowClose.mockReturnValueOnce(firstFlush.promise);
    const appWindow = installWindowMock();

    await act(async () => {
      root?.render(<Harness />);
      await Promise.resolve();
    });

    const firstPreventDefault = vi.fn();
    const secondPreventDefault = vi.fn();
    const closeListener = appWindow.getCloseListener();
    const firstCloseRequest = Promise.resolve(
      closeListener({ preventDefault: firstPreventDefault }),
    );

    await vi.waitFor(() => {
      expect(mocks.flushBeforeWindowClose).toHaveBeenCalledOnce();
    });

    const secondCloseRequest = Promise.resolve(
      closeListener({ preventDefault: secondPreventDefault }),
    );
    await secondCloseRequest;
    const closeCallsWhileFlushPending = appWindow.close.mock.calls.length;

    firstFlush.resolve(undefined);
    await firstCloseRequest;

    expect(firstPreventDefault).toHaveBeenCalledOnce();
    expect(secondPreventDefault).toHaveBeenCalledOnce();
    expect(mocks.confirm3).toHaveBeenCalledOnce();
    expect(mocks.saveAllDirtyGraphs).toHaveBeenCalledOnce();
    expect(mocks.flushBeforeWindowClose).toHaveBeenCalledOnce();
    expect(closeCallsWhileFlushPending).toBe(0);
    expect(appWindow.close).toHaveBeenCalledOnce();
    expect(appWindow.getRecursivePreventDefault()).not.toHaveBeenCalled();
  });
});
