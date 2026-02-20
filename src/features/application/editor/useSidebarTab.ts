import { useCallback } from 'react';
import { useLayoutStore } from '@/features/core/layout/layoutStore';

/**
 * Hook to get switchSidebarTab function
 * 
 * 提供统一的 switchSidebarTab 函数访问，避免在多个管理 hooks 中重复传参
 */
export function useSidebarTab() {
  const switchSidebarTab = useCallback((tab: 'graphs' | 'variables' | 'data') => {
    const layoutStore = useLayoutStore.getState();
    const sidebarNode = layoutStore.nodes['sidebar'];
    if (sidebarNode) {
      layoutStore.updateNode('sidebar', {
        data: { ...sidebarNode.data, visible: true, currentTab: tab }
      });
      if ((sidebarNode.pixelSize || 0) < 50) {
        layoutStore.updateNode('sidebar', { pixelSize: 260 });
      }
    }
  }, []);

  return switchSidebarTab;
}
