// @vitest-environment happy-dom

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useWorkbenchStore } from '@/features/core/workbench/workbenchStore';
import { activateSidebarTab } from '@/features/application/editor/useSidebarTab';
import { revealWorkbenchView } from '@/features/application/layout/workbenchLayoutActions';

vi.mock('@/features/application/layout/workbenchLayoutActions', () => ({
  revealWorkbenchView: vi.fn(async () => null),
}));

describe('Activity Bar sidebar activation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useWorkbenchStore.getState().resetWorkbenchUIState();
  });

  it('sets the current tab and reveals Resources on every selection', async () => {
    await activateSidebarTab('project');
    await activateSidebarTab('project');

    expect(useWorkbenchStore.getState().sidebarCurrentTab).toBe('project');
    expect(revealWorkbenchView).toHaveBeenNthCalledWith(1, 'resources');
    expect(revealWorkbenchView).toHaveBeenNthCalledWith(2, 'resources');
  });
});
