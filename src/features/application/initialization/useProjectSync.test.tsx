// @vitest-environment happy-dom
import { act, StrictMode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { listen } from '@tauri-apps/api/event';
import { useProjectSync } from './useProjectSync';

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}));

vi.mock('@/features/core/dataStore', () => ({
  loadActivatedProject: vi.fn(async () => null),
  useProjectIOStore: {
    getState: () => ({
      loadProject: async () => null,
      refreshResourceIndex: async () => false,
      loadGraph: async () => false,
    }),
    setState: vi.fn(),
  },
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function Harness(): null {
  useProjectSync();
  return null;
}

describe('useProjectSync', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.clearAllMocks();
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    host.remove();
  });

  it('starts the service-owned project event stream without hydrating project state', async () => {
    const unlisten = vi.fn();
    vi.mocked(listen).mockResolvedValue(unlisten);

    await act(async () => {
      root.render(<Harness />);
      await Promise.resolve();
    });

    expect(listen).toHaveBeenCalledOnce();
  });

  it('keeps one project listener when StrictMode cleanup races async startup', async () => {
    const startup = deferred<() => void>();
    const unlisten = vi.fn();
    vi.mocked(listen).mockReturnValue(startup.promise);

    await act(async () => {
      root.render(
        <StrictMode>
          <Harness />
        </StrictMode>,
      );
      await Promise.resolve();
    });

    expect(listen).toHaveBeenCalledOnce();

    await act(async () => {
      startup.resolve(unlisten);
      await startup.promise;
      await Promise.resolve();
    });

    expect(listen).toHaveBeenCalledOnce();

    await act(async () => root.unmount());
    expect(unlisten).toHaveBeenCalledOnce();
  });

  it('keeps a shared listener until the final consumer unmounts', async () => {
    const unlisten = vi.fn();
    vi.mocked(listen).mockResolvedValue(unlisten);

    await act(async () => {
      root.render(
        <>
          <Harness />
          <Harness />
        </>,
      );
      await Promise.resolve();
    });

    expect(listen).toHaveBeenCalledOnce();

    await act(async () => {
      root.render(<Harness />);
      await Promise.resolve();
    });
    expect(unlisten).not.toHaveBeenCalled();

    await act(async () => root.unmount());
    expect(unlisten).toHaveBeenCalledOnce();
  });

  it('releases a listener that finishes startup after every consumer unmounts', async () => {
    const startup = deferred<() => void>();
    const unlisten = vi.fn();
    vi.mocked(listen).mockReturnValue(startup.promise);

    await act(async () => {
      root.render(<Harness />);
      await Promise.resolve();
    });
    expect(listen).toHaveBeenCalledOnce();

    await act(async () => root.unmount());
    expect(unlisten).not.toHaveBeenCalled();

    await act(async () => {
      startup.resolve(unlisten);
      await startup.promise;
      await Promise.resolve();
    });

    expect(listen).toHaveBeenCalledOnce();
    expect(unlisten).toHaveBeenCalledOnce();
  });

  it('handles stream startup failure and permits a later retry', async () => {
    const startup = deferred<() => void>();
    const startupError = new Error('stream unavailable');
    vi.mocked(listen).mockReturnValueOnce(startup.promise);

    await act(async () => {
      root.render(<Harness />);
      await Promise.resolve();
      startup.reject(startupError);
      await startup.promise.catch(() => undefined);
      await Promise.resolve();
    });

    const unlisten = vi.fn();
    vi.mocked(listen).mockResolvedValueOnce(unlisten);
    await act(async () => {
      root.render(null);
      await Promise.resolve();
    });
    await act(async () => {
      root.render(<Harness />);
      await Promise.resolve();
    });

    expect(listen).toHaveBeenCalledTimes(2);
    await act(async () => root.unmount());
    expect(unlisten).toHaveBeenCalledOnce();
  });
});
