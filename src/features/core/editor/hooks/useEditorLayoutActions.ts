/**
 * 编辑器布局操作：setActiveGroup、switchSidebarTab
 */
import { useCallback } from 'react';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import {
  useSidebarTab,
  type SidebarTabId,
} from '@/features/application/editor/useSidebarTab';

export function useEditorLayoutActions() {
  const setActiveGroup = useLayoutStore((s) => s.setActiveGroup);
  const showSidebarTab = useSidebarTab();

  const setActiveGroupId = useCallback((id: string) => {
    useLayoutStore.getState().setActiveGroup(id);
  }, []);

  const switchSidebarTab = useCallback((tab: SidebarTabId) => {
    showSidebarTab(tab);
  }, [showSidebarTab]);

  return { setActiveGroup, setActiveGroupId, switchSidebarTab };
}
