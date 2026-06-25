import { useCallback } from 'react';
import { useLayoutStore } from '@/features/core/layout/layoutStore';

export type SidebarTabId = 'graphs' | 'variables' | 'data' | 'commands' | 'charts';

export function useSidebarTab() {
  const switchSidebarTab = useCallback((tab: SidebarTabId) => {
    const layoutStore = useLayoutStore.getState();
    const sidebarNode = layoutStore.nodes['sidebar'];
    if (sidebarNode) {
      layoutStore.updateNode('sidebar', {
        data: { ...sidebarNode.data, visible: true, currentTab: tab },
      });
      if ((sidebarNode.pixelSize || 0) < 50) {
        layoutStore.updateNode('sidebar', { pixelSize: 260 });
      }
    }
  }, []);

  return switchSidebarTab;
}
