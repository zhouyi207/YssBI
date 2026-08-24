import { useCallback } from 'react';
import { revealWorkbenchView } from '@/features/application/layout/workbenchLayoutActions';
import { useWorkbenchStore } from '@/features/core/workbench/workbenchStore';
import type { SidebarTabId } from '@/features/core/workbench/workbenchTypes';

export type { SidebarTabId };

export async function activateSidebarTab(tab: SidebarTabId): Promise<void> {
  useWorkbenchStore.getState().setSidebarCurrentTab(tab);
  await revealWorkbenchView('resources');
}

export function useSidebarTab() {
  return useCallback((tab: SidebarTabId) => activateSidebarTab(tab), []);
}
