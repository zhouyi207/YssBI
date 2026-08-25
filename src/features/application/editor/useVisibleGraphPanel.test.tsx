// @vitest-environment happy-dom

import { act, createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { useVisibleGraphPanel } from './useVisibleGraphPanel';

const mocks = vi.hoisted(() => ({
  synchronizeVisibleGraphPanel: vi.fn(async () => true),
}));

vi.mock('./synchronizeVisibleGraphPanel', () => ({
  synchronizeVisibleGraphPanel: mocks.synchronizeVisibleGraphPanel,
}));

type VisibilityListener = (event: { isVisible: boolean }) => void;
type GroupListener = (event: object) => void;

function createPanelApi() {
  const visibilityListeners = new Set<VisibilityListener>();
  const groupListeners = new Set<GroupListener>();
  return {
    isVisible: false,
    onDidVisibilityChange(listener: VisibilityListener) {
      visibilityListeners.add(listener);
      return { dispose: () => visibilityListeners.delete(listener) };
    },
    onDidGroupChange(listener: GroupListener) {
      groupListeners.add(listener);
      return { dispose: () => groupListeners.delete(listener) };
    },
    emitVisibilityChange() {
      for (const listener of visibilityListeners) listener({ isVisible: true });
    },
    emitGroupChange() {
      for (const listener of groupListeners) listener({});
    },
  };
}

describe('useVisibleGraphPanel', () => {
  let host: HTMLDivElement;
  let root: Root;
  let api: ReturnType<typeof createPanelApi>;
  let scope = { groupId: 'group-1', graphPath: 'events/Main.yssbi-event' };

  function Harness() {
    useVisibleGraphPanel(api, scope);
    return null;
  }

  beforeEach(() => {
    vi.clearAllMocks();
    api = createPanelApi();
    scope = { groupId: 'group-1', graphPath: 'events/Main.yssbi-event' };
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it('waits for Dockview visibility before synchronizing a graph panel', async () => {
    await act(async () => {
      root.render(createElement(Harness));
      await Promise.resolve();
    });

    expect(mocks.synchronizeVisibleGraphPanel).not.toHaveBeenCalled();

    await act(async () => {
      api.isVisible = true;
      api.emitVisibilityChange();
      await Promise.resolve();
    });

    expect(mocks.synchronizeVisibleGraphPanel).toHaveBeenCalledWith(scope);
  });
});
