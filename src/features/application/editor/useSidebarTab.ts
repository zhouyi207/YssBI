import { useCallback } from 'react';
import { showSidebarTab as persistShowSidebarTab } from '@/features/core/layout/workbenchLayoutService';
import type { SidebarTabId } from '@/features/core/workbench';

export type { SidebarTabId };

export function useSidebarTab() {
  return useCallback((tab: SidebarTabId) => {
    persistShowSidebarTab(tab);
  }, []);
}
