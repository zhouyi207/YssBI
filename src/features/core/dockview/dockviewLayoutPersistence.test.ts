import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  editorUnbind: vi.fn(),
  editorWhenReady: vi.fn(),
  editorRestore: vi.fn(),
  panelUnbind: vi.fn(),
  panelWhenReady: vi.fn(),
  panelRestore: vi.fn(),
  workbenchRestore: vi.fn(),
  setWorkbenchState: vi.fn(),
}));

vi.mock('@/features/core/workbench', () => ({
  workbenchGridPort: {
    restore: mocks.workbenchRestore,
    serialize: vi.fn(),
  },
  useWorkbenchStore: {
    getState: vi.fn(() => ({})),
    setState: mocks.setWorkbenchState,
  },
}));

vi.mock('./dockviewEditorPort', () => ({
  editorDockviewPort: {
    unbind: mocks.editorUnbind,
    whenReady: mocks.editorWhenReady,
    restore: mocks.editorRestore,
    serialize: vi.fn(),
    isReady: true,
  },
}));

vi.mock('./panelDockviewPort', () => ({
  panelDockviewPort: {
    unbind: mocks.panelUnbind,
    whenReady: mocks.panelWhenReady,
    restore: mocks.panelRestore,
    serialize: vi.fn(),
    isCollapsed: vi.fn(() => false),
    isReady: true,
  },
}));

import {
  dockviewLayoutStorageKey,
  hydrateDockviewLayout,
  invalidateDockviewLayoutHydration,
} from './dockviewLayoutPersistence';

describe('dockview layout hydration', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.panelWhenReady.mockResolvedValue(undefined);
    mocks.panelRestore.mockResolvedValue(undefined);
    mocks.editorWhenReady.mockResolvedValue(undefined);
    const values = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => values.set(key, value),
      removeItem: (key: string) => values.delete(key),
    });
    localStorage.setItem(dockviewLayoutStorageKey(), JSON.stringify({
      workbench: { grid: {} },
      editor: { grid: {}, panels: {} },
      shell: { grid: {}, panels: {} },
      preferences: { sidebarUserHidden: true },
    }));
  });

  it('does not apply stale preferences after reset invalidates hydration', async () => {
    let finishRestore: (() => void) | undefined;
    mocks.editorRestore.mockReturnValue(new Promise<void>((resolve) => {
      finishRestore = resolve;
    }));

    const hydration = hydrateDockviewLayout();
    invalidateDockviewLayoutHydration();
    finishRestore?.();

    await expect(hydration).resolves.toBe(false);
    expect(mocks.workbenchRestore).toHaveBeenCalledOnce();
    expect(mocks.panelRestore).toHaveBeenCalledOnce();
    expect(mocks.setWorkbenchState).not.toHaveBeenCalled();
  });
});
