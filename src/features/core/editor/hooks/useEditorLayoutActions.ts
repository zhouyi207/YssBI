/**
 * 编辑器布局操作：setActiveGroup、switchSidebarTab
 */
import { useCallback } from 'react';
import { useLayoutStore } from '@/features/core/layout/layoutStore';

export function useEditorLayoutActions() {
  const setActiveGroup = useLayoutStore((s) => s.setActiveGroup);

  const setActiveGroupId = useCallback((id: string) => {
    useLayoutStore.getState().setActiveGroup(id);
  }, []);

  const switchSidebarTab = useCallback((tab: 'graphs' | 'variables' | 'data') => {
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

  return { setActiveGroup, setActiveGroupId, switchSidebarTab };
}
