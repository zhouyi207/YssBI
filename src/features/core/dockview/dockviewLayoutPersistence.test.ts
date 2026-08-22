import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  editorUnbind: vi.fn(),
  editorWhenReady: vi.fn(),
  editorRestore: vi.fn(),
  editorSerialize: vi.fn(),
  panelUnbind: vi.fn(),
  panelWhenReady: vi.fn(),
  panelRestore: vi.fn(),
  panelSerialize: vi.fn(),
  workbenchRestore: vi.fn(),
  workbenchSerialize: vi.fn(),
  getWorkbenchState: vi.fn(),
  setWorkbenchState: vi.fn(),
}));

vi.mock('@/features/core/workbench', () => ({
  isSidebarTabId: (value: unknown) => (
    typeof value === 'string'
    && ['project', 'nodes', 'data', 'commands'].includes(value)
  ),
  workbenchGridPort: {
    restore: mocks.workbenchRestore,
    serialize: mocks.workbenchSerialize,
  },
  useWorkbenchStore: {
    getState: mocks.getWorkbenchState,
    setState: mocks.setWorkbenchState,
  },
}));

vi.mock('./dockviewEditorPort', () => ({
  editorDockviewPort: {
    unbind: mocks.editorUnbind,
    whenReady: mocks.editorWhenReady,
    restore: mocks.editorRestore,
    serialize: mocks.editorSerialize,
    isReady: true,
  },
}));

vi.mock('./panelDockviewPort', () => ({
  panelDockviewPort: {
    unbind: mocks.panelUnbind,
    whenReady: mocks.panelWhenReady,
    restore: mocks.panelRestore,
    serialize: mocks.panelSerialize,
    isReady: true,
  },
}));

import {
  dockviewLayoutStorageKey,
  hydrateDockviewLayout,
  invalidateDockviewLayoutHydration,
  persistDockviewLayoutNow,
} from './dockviewLayoutPersistence';

describe('dockview layout hydration', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.panelWhenReady.mockResolvedValue(undefined);
    mocks.panelRestore.mockResolvedValue(undefined);
    mocks.editorWhenReady.mockResolvedValue(undefined);
    mocks.getWorkbenchState.mockReturnValue({
      sidebarCurrentTab: 'project',
      sidebarUserHidden: true,
      panelCollapsed: true,
      detailUserHidden: false,
      isSettingsOpen: true,
      isNodeDocumentationOpen: true,
      zenMode: true,
    });
    const values = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => values.set(key, value),
      removeItem: (key: string) => values.delete(key),
    });
    localStorage.setItem(dockviewLayoutStorageKey(), JSON.stringify({
      workbench: { grid: {} },
      editor: { grid: {}, panels: {} },
      shell: {
        grid: {},
        panels: {},
        edgeGroups: {
          bottom: { size: 320, visible: true, collapsed: true },
        },
      },
      preferences: { sidebarCurrentTab: 'variables', sidebarUserHidden: true },
    }));
  });

  it('persists edge-group collapse in the shell layout, not preferences', async () => {
    const workbench = { grid: { root: { type: 'branch', data: [] } } };
    const editor = { grid: {}, panels: {} };
    const shell = {
      grid: {},
      panels: {},
      edgeGroups: {
        bottom: { size: 320, visible: true, collapsed: true },
      },
    };
    mocks.workbenchSerialize.mockReturnValue(workbench);
    mocks.editorSerialize.mockResolvedValue(editor);
    mocks.panelSerialize.mockResolvedValue(shell);

    await persistDockviewLayoutNow();

    const persisted = JSON.parse(localStorage.getItem(dockviewLayoutStorageKey()) ?? '{}');
    expect(persisted.shell).toEqual(shell);
    expect(persisted.preferences).not.toHaveProperty('panelCollapsed');
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
    expect(mocks.panelRestore).toHaveBeenCalledWith(expect.objectContaining({
      edgeGroups: {
        bottom: { size: 320, visible: true, collapsed: true },
      },
    }));
    expect(mocks.setWorkbenchState).not.toHaveBeenCalled();
  });

  it('falls back to Project when persisted preferences contain a removed sidebar tab', async () => {
    await expect(hydrateDockviewLayout()).resolves.toBe(true);

    expect(mocks.setWorkbenchState).toHaveBeenCalledWith(expect.objectContaining({
      sidebarCurrentTab: 'project',
      sidebarUserHidden: true,
    }));
  });
});
