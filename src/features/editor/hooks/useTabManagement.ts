import { useCallback } from 'react';
import { Graph } from '@/shared/types/editor';
import { useNodeStore } from '@/features/node-registry/stores';
import { useProjectStore } from '@/features/project';
import { useLayoutStore, LayoutState } from '@/features/editor/stores/layoutStore';
import { useEditorStore } from '../stores';
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
    setActiveTabId(newId, targetGroupId);
    if (!newId) return;

    const id = newId;
    const tabState = useNodeStore.getState().tabs[id];

    if (!tabState) {
      const st = useProjectStore.getState();
      const source = initialData || st.events[id] || st.functions[id] || st.macros[id];
      if (source) {
        const { nodes: n, variables: v } = deserializeSubGraph(source);
        useNodeStore.getState().initTab(id, n, v);
      } else {
        useNodeStore.getState().initTab(id, [], {});
      }
    }

    const st = useProjectStore.getState();
    const tabSource = st.events[id] || st.functions[id] || st.macros[id];
    const type = forceType || (tabSource as any)?.type;
    if (type) setSelectedInfo(id, type as any);
  }, [setActiveTabId, setSelectedInfo]);

  const openSubGraph = useCallback((
    id: string,
    name: string,
    type: "event" | "function" | "macro",
    initialData?: Graph
  ) => {
    const layoutStore = useLayoutStore.getState();
    const targetGroupId = layoutStore.activeEditorGroupId || layoutStore.activeGroupId || 'default_editor';

    layoutStore.addTab(targetGroupId, {
      id,
      title: name,
      component: 'GraphEditor',
      type
    });

    layoutStore.setActiveGroup(targetGroupId);
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
