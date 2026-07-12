import { describe, expect, it, vi } from 'vitest';
import { buildViewMenuItems } from './menubarViewItems';

const t = ((key: string) => key) as never;

describe('buildViewMenuItems', () => {
  it('maps secondary side bar toggle to detail visibility action', () => {
    const toggleDetail = vi.fn();
    const items = buildViewMenuItems(
      t,
      {
        isSidebarVisible: true,
        isDetailVisible: false,
        isLogPanelVisible: true,
        zenMode: false,
      },
      {
        toggleSidebar: vi.fn(),
        toggleDetail,
        toggleLogPanel: vi.fn(),
        toggleZenMode: vi.fn(),
        resetLayout: vi.fn(),
      },
    );

    const detailItem = items.find((item) => item.label === 'menubar.showSecondarySideBar');
    expect(detailItem?.shortcut).toBe('Ctrl+I');
    detailItem?.onClick?.();
    expect(toggleDetail).toHaveBeenCalledOnce();
  });

  it('includes reset layout at the end of the View menu', () => {
    const resetLayout = vi.fn();
    const items = buildViewMenuItems(
      t,
      {
        isSidebarVisible: true,
        isDetailVisible: true,
        isLogPanelVisible: true,
        zenMode: false,
      },
      {
        toggleSidebar: vi.fn(),
        toggleDetail: vi.fn(),
        toggleLogPanel: vi.fn(),
        toggleZenMode: vi.fn(),
        resetLayout,
      },
    );

    const resetItem = items[items.length - 1];
    expect(resetItem?.label).toBe('menubar.resetLayout');
    resetItem?.onClick?.();
    expect(resetLayout).toHaveBeenCalledOnce();
  });
});
