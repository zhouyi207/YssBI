import { useCallback } from 'react';
import { Graph } from '@/shared/types/domain';
import { useNodeStore } from '@/features/core/node-registry/stores';
import { useProjectStore } from '@/features/core/project';
import { useLayoutStore, LayoutState } from '@/features/application/editor/core/stores/layoutStore';
import { useEditorStore } from '../../core/stores';
import { deserializeSubGraph } from '@/shared/utils/editor';

/**
 * Tab Management Hook
 * Handles opening, closing, and switching between tabs
 */
export function useTabManagement() {
  const { setSelectedInfo } = useEditorStore();
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
    forceType?: 'event' | 'function' | 'macro' | 'setting',
    initialData?: Graph,
    targetGroupId?: string
  ) => {
    console.log('[useTabManagement.handleSetActiveTabId] Called with:', { newId, forceType, initialData, targetGroupId });
    
    setActiveTabId(newId, targetGroupId);
    if (!newId) return;

    const id = newId;
    const tabState = useNodeStore.getState().tabs[id];
    
    console.log('[useTabManagement.handleSetActiveTabId] Tab state exists:', !!tabState);

    if (!tabState) {
      const st = useProjectStore.getState();
      // 从 graphs 中获取数据
      const source = initialData || st.graphs[id];
      
      console.log('[useTabManagement.handleSetActiveTabId] Source data:', source);
      
      if (source) {
        const { nodes: n, variables: v } = deserializeSubGraph(source);
        console.log('[useTabManagement.handleSetActiveTabId] Deserialized:', { nodes: n.length, variables: Object.keys(v).length });
        useNodeStore.getState().initTab(id, n, v);
      } else {
        console.log('[useTabManagement.handleSetActiveTabId] No source data, initializing empty tab');
        useNodeStore.getState().initTab(id, [], {});
      }
    }

    const st = useProjectStore.getState();
    // 从 graphs 中获取数据
    const tabSource = st.graphs[id];
    const type = forceType || (tabSource as any)?.type;
    
    console.log('[useTabManagement.handleSetActiveTabId] Final type:', type, 'tabSource:', tabSource);
    
    if (type) setSelectedInfo(id, type as any);
  }, [setActiveTabId, setSelectedInfo]);

  const openSubGraph = useCallback((
    id: string,
    name: string,
    type: "event" | "function" | "macro",
    initialData?: Graph
  ) => {
    console.log('[useTabManagement.openSubGraph] Called with:', { id, name, type, initialData });
    
    const layoutStore = useLayoutStore.getState();
    const targetGroupId = layoutStore.activeEditorGroupId || layoutStore.activeGroupId || 'default_editor';
    
    console.log('[useTabManagement.openSubGraph] Target group:', targetGroupId);

    layoutStore.addTab(targetGroupId, {
      id,
      title: name,
      component: 'GraphEditor',
      type
    });

    console.log('[useTabManagement.openSubGraph] Tab added, setting active group');
    layoutStore.setActiveGroup(targetGroupId);
    
    console.log('[useTabManagement.openSubGraph] Calling handleSetActiveTabId');
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
    openSubGraph,
    openSettingsTab,
    closeTab,
    splitEditorRight,
    closeGroup,
  };
}
