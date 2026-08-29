// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { currentAppWindow } from '@/services/platform/appWindow';
import type { AppWindowHandle } from '@/services/platform/appWindow';
import { WorkbenchLayoutError } from '@/features/core/dockview/workbenchTypes';
import { useEditorWindowCloseGuard } from './useEditorWindowCloseGuard';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

type CloseListener = () => Promise<'allow' | 'prevent'>;

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

vi.mock('@/services/platform/appWindow', () => ({
  currentAppWindow: vi.fn(),
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
vi.mock('@/features/application/observability/appLogger', () => ({
  logger: { app: { error: mocks.logError, warn: mocks.logWarn } },
}));

function Harness(): null {
  useEditorWindowCloseGuard();
  return null;
}

function installWindowMock() {
  let closeListener: CloseListener | undefined;
  let recursiveDecision: 'allow' | 'prevent' | undefined;
  const unlisten = vi.fn();
  const close = vi.fn(async () => {
    recursiveDecision = await closeListener?.();
    return { ok: true, value: undefined } as const;
  });
  const onCloseRequested = vi.fn(async (listener: CloseListener) => {
    closeListener = listener;
    return { ok: true, value: unlisten } as const;
  });
  vi.mocked(currentAppWindow).mockReturnValue({
    label: 'main',
    close,
    onCloseRequested,
  } as unknown as AppWindowHandle);

  return {
    close,
    onCloseRequested,
    unlisten,
    getCloseListener(): CloseListener {
      if (!closeListener) throw new Error('close listener is not attached');
      return closeListener;
    },
    getRecursiveDecision: () => recursiveDecision,
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

    await act(async () => {
      await appWindow.getCloseListener()();
    });

    expect(mocks.confirm3).toHaveBeenCalledOnce();
    expect(mocks.saveAllDirtyGraphs).not.toHaveBeenCalled();
    expect(mocks.flushBeforeWindowClose).toHaveBeenCalledOnce();
    expect(appWindow.close).toHaveBeenCalledOnce();
    expect(appWindow.getRecursiveDecision()).toBe('allow');

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

    const closeRequest = appWindow.getCloseListener()();
    await Promise.resolve();

    expect(mocks.confirm3).not.toHaveBeenCalled();
    expect(mocks.flushBeforeWindowClose).toHaveBeenCalledOnce();
    expect(appWindow.close).not.toHaveBeenCalled();

    hydrationAndFlush.resolve(undefined);
    await closeRequest;

    expect(appWindow.close).toHaveBeenCalledOnce();
    expect(appWindow.getRecursiveDecision()).toBe('allow');
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

    await appWindow.getCloseListener()();

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

    const closeListener = appWindow.getCloseListener();
    const firstCloseRequest = closeListener();

    await vi.waitFor(() => {
      expect(mocks.flushBeforeWindowClose).toHaveBeenCalledOnce();
    });

    const secondCloseRequest = closeListener();
    await secondCloseRequest;
    const closeCallsWhileFlushPending = appWindow.close.mock.calls.length;

    firstFlush.resolve(undefined);
    await firstCloseRequest;

    expect(mocks.confirm3).toHaveBeenCalledOnce();
    expect(mocks.saveAllDirtyGraphs).toHaveBeenCalledOnce();
    expect(mocks.flushBeforeWindowClose).toHaveBeenCalledOnce();
    expect(closeCallsWhileFlushPending).toBe(0);
    expect(appWindow.close).toHaveBeenCalledOnce();
    expect(appWindow.getRecursiveDecision()).toBe('allow');
  });
});
