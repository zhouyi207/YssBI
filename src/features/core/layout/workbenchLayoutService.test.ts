import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  editorUnbind: vi.fn(),
  editorWhenReady: vi.fn(),
  editorReset: vi.fn(),
  panelUnbind: vi.fn(),
  panelWhenReady: vi.fn(),
  panelGetSnapshot: vi.fn(),
  panelSetCollapsed: vi.fn(),
  panelSetPosition: vi.fn(),
  invalidateHydration: vi.fn(),
  persist: vi.fn(),
  resetUI: vi.fn(),
  resetGrid: vi.fn(),
  setPartSize: vi.fn(),
}));

vi.mock('@/features/core/dockview', () => ({
  editorDockviewPort: {
    unbind: mocks.editorUnbind,
    whenReady: mocks.editorWhenReady,
    reset: mocks.editorReset,
  },
  panelDockviewPort: {
    unbind: mocks.panelUnbind,
    whenReady: mocks.panelWhenReady,
    getSnapshot: mocks.panelGetSnapshot,
    setCollapsed: mocks.panelSetCollapsed,
    setPosition: mocks.panelSetPosition,
    getPosition: vi.fn(() => 'bottom'),
    activate: vi.fn(),
  },
  invalidateDockviewLayoutHydration: mocks.invalidateHydration,
  persistDockviewLayoutDebounced: mocks.persist,
}));

vi.mock('@/features/core/workbench', () => ({
  DEFAULT_WORKBENCH_PANEL_SIZE: 200,
  WORKBENCH_PANEL_PART_ID: 'panel',
  useWorkbenchStore: {
    getState: () => ({
      zenMode: false,
      resetWorkbenchUIState: mocks.resetUI,
    }),
  },
  workbenchGridPort: {
    resetToDefault: mocks.resetGrid,
    setPartSize: mocks.setPartSize,
  },
}));

import {
  resetWorkbenchLayout,
  togglePanelCollapsed,
} from './workbenchLayoutService';

describe('workbench bottom panel layout', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.panelWhenReady.mockResolvedValue(undefined);
    mocks.editorWhenReady.mockResolvedValue(undefined);
    mocks.editorReset.mockResolvedValue(undefined);
    mocks.panelGetSnapshot.mockReturnValue({
      revision: 1,
      ready: true,
      collapsed: false,
    });
    mocks.panelSetPosition.mockResolvedValue(true);
    mocks.panelSetCollapsed.mockResolvedValue(true);
  });

  it('collapses through the Dockview edge group without resizing the workbench leaf', () => {
    togglePanelCollapsed();

    expect(mocks.panelGetSnapshot).toHaveBeenCalledOnce();
    expect(mocks.panelSetCollapsed).toHaveBeenCalledWith(true);
    expect(mocks.setPartSize).not.toHaveBeenCalled();
    expect(mocks.persist).toHaveBeenCalledOnce();
  });

  it('rebuilds the shell before resetting the nested editor layout', async () => {
    await resetWorkbenchLayout('right');

    expect(mocks.invalidateHydration).toHaveBeenCalledOnce();
    expect(mocks.resetUI).toHaveBeenCalledOnce();
    expect(mocks.panelUnbind).toHaveBeenCalledOnce();
    expect(mocks.editorUnbind).toHaveBeenCalledOnce();
    expect(mocks.resetGrid).toHaveBeenCalledOnce();
    expect(mocks.panelWhenReady).toHaveBeenCalledOnce();
    expect(mocks.editorWhenReady).toHaveBeenCalledOnce();
    expect(mocks.editorReset).toHaveBeenCalledOnce();
    expect(mocks.panelSetPosition).toHaveBeenCalledWith('right');
    expect(mocks.panelSetCollapsed).toHaveBeenCalledWith(false);
    expect(mocks.persist).toHaveBeenCalledOnce();
  });
});
