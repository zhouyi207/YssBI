import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  editorRestore: vi.fn(),
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
    restore: mocks.editorRestore,
    serialize: vi.fn(),
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
    const values = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => values.set(key, value),
      removeItem: (key: string) => values.delete(key),
    });
    localStorage.setItem(dockviewLayoutStorageKey(), JSON.stringify({
      version: 1,
      workbench: { grid: {} },
      editor: { grid: {}, panels: {} },
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
    expect(mocks.setWorkbenchState).not.toHaveBeenCalled();
  });
});
