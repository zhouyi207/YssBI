import { useCallback } from 'react';
import { Graph } from '@/shared/types/domain';
import { getGraphById, useProjectIOStore } from '@/features/core/dataStore';
import { useLayoutStore, LayoutState } from '@/features/core/layout/layoutStore';
import { useEditorStore } from '@/features/core/editor';
import { logger } from '@/utils/appLogger';

/**
 * Tab Management Hook
 * Handles opening, closing, and switching between tabs
 */
export function useTabManagement() {
  const setSelectedInfo = useEditorStore((s) => s.setSelectedInfo);
  const activeGroupId = useLayoutStore((s: LayoutState) => s.activeGroupId);
  // const activeEditorGroupId = useLayoutStore((s: LayoutState) => s.activeEditorGroupId);

  const setActiveTabId = useCallback((id: string | null, targetGroupId?: string) => {
    const groupId = targetGroupId || activeGroupId;
    if (groupId) {
      useLayoutStore.getState().updateNode(groupId, {
        data: {
          ...useLayoutStore.getState().nodes[groupId].data,
          activeTabId: id || undefined
        }
      });
    }
  }, [activeGroupId]);

  const handleSetActiveTabId = useCallback((
    newId: string | null,
    forceType?: 'event' | 'function' | 'setting',
    initialData?: Graph,
    targetGroupId?: string
  ) => {
    logger.graph.trace(`handleSetActiveTabId called: newId=${newId}, forceType=${forceType}, targetGroupId=${targetGroupId}`, 'TabManagement');
    
    setActiveTabId(newId, targetGroupId);
    if (!newId) return;

    const tabSource = initialData || getGraphById(newId);
    const type = forceType || (tabSource as any)?.type;
    
    logger.graph.trace(`handleSetActiveTabId final type: ${type}`, 'TabManagement');
    
    if (type) setSelectedInfo(newId, type as any);
  }, [setActiveTabId, setSelectedInfo]);

  const openGraph = useCallback(async (
    id: string,
    name: string,
    type: "event" | "function",
    initialData?: Graph
  ) => {
    logger.graph.trace(`openGraph called: id=${id}, name=${name}, type=${type}`, 'TabManagement');
    if (!initialData) {
      const loaded = await useProjectIOStore.getState().loadGraph(id);
      if (!loaded) return;
    }
    
    const layoutStore = useLayoutStore.getState();
    const targetGroupId = layoutStore.activeEditorGroupId || layoutStore.activeGroupId || 'default_editor';
    
    logger.graph.trace(`openGraph target group: ${targetGroupId}`, 'TabManagement');

    layoutStore.addTab(targetGroupId, {
      id,
      title: name,
      component: 'GraphEditor',
      type
    });

    logger.graph.trace('Tab added, setting active group', 'TabManagement');
    layoutStore.setActiveGroup(targetGroupId);
    
    logger.graph.trace('Calling handleSetActiveTabId', 'TabManagement');
    handleSetActiveTabId(id, type, initialData, targetGroupId);
  }, [handleSetActiveTabId]);

  const openSettingsTab = useCallback(() => {
    const layoutStore = useLayoutStore.getState();
    const targetGroupId = layoutStore.activeEditorGroupId || 'default_editor';
    layoutStore.openSettings();
    handleSetActiveTabId("settings", "setting", undefined, targetGroupId);
  }, [handleSetActiveTabId]);

  const closeTab = useCallback((id: string, e?: React.MouseEvent) => {
    if (e) e.stopPropagation();
    const nodes = useLayoutStore.getState().nodes;
    const node = Object.values(nodes).find(n => n.data?.tabs?.find(t => t.id === id));
    if (node) {
      useLayoutStore.getState().removeTab(node.id, id);
    }
  }, []);

  const splitEditorRight = useCallback((sourceGroupId: string) => {
    useLayoutStore.getState().splitNode(sourceGroupId, 'row', 'GraphEditor');
  }, []);

  const closeGroup = useCallback((id: string) => {
    useLayoutStore.getState().removeNode(id);
  }, []);

  return {
    setActiveTabId,
    openGraph,
    openSettingsTab,
    closeTab,
    splitEditorRight,
    closeGroup,
  };
}
