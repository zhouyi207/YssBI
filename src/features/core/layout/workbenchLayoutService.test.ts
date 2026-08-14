import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  editorReset: vi.fn(),
  invalidateHydration: vi.fn(),
  persist: vi.fn(),
  resetUI: vi.fn(),
  resetGrid: vi.fn(),
  movePart: vi.fn(),
}));

vi.mock('@/features/core/dockview', () => ({
  editorDockviewPort: { reset: mocks.editorReset },
  invalidateDockviewLayoutHydration: mocks.invalidateHydration,
  persistDockviewLayoutDebounced: mocks.persist,
  persistDockviewLayoutNow: vi.fn(),
}));

vi.mock('@/features/core/workbench', () => ({
  DEFAULT_WORKBENCH_PANEL_SIZE: 200,
  WORKBENCH_EDITOR_PART_ID: 'editor',
  WORKBENCH_PANEL_PART_ID: 'panel',
  useWorkbenchStore: {
    getState: () => ({ resetWorkbenchUIState: mocks.resetUI }),
  },
  workbenchGridPort: {
    resetToDefault: mocks.resetGrid,
    movePart: mocks.movePart,
  },
}));

import { resetWorkbenchLayout } from './workbenchLayoutService';

describe('resetWorkbenchLayout', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('waits for the editor reset before restoring and positioning the workbench grid', async () => {
    let finishEditorReset: (() => void) | undefined;
    mocks.editorReset.mockReturnValue(new Promise<void>((resolve) => {
      finishEditorReset = resolve;
    }));

    const reset = resetWorkbenchLayout('right');

    expect(mocks.invalidateHydration).toHaveBeenCalledOnce();
    expect(mocks.resetUI).toHaveBeenCalledOnce();
    expect(mocks.resetGrid).not.toHaveBeenCalled();

    finishEditorReset?.();
    await reset;

    expect(mocks.resetGrid).toHaveBeenCalledOnce();
    expect(mocks.movePart).toHaveBeenCalledWith('panel', 'right', 'editor', 200);
    expect(mocks.persist).toHaveBeenCalledOnce();
  });

  it('keeps the canonical bottom placement without an extra move', async () => {
    mocks.editorReset.mockResolvedValue(undefined);

    await resetWorkbenchLayout('bottom');

    expect(mocks.resetGrid).toHaveBeenCalledOnce();
    expect(mocks.movePart).not.toHaveBeenCalled();
  });
});
