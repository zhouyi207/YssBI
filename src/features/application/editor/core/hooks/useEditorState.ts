import { useMemo } from 'react';
import { useLayoutStore, LayoutState } from '@/features/core/layout/layoutStore';
import { useProjectStore } from '@/features/core/project';
import { useTabNodes, useTabVariables } from '@/features/core/_node/useNodeStore';
import { useEditorStore } from '../stores';
import { useShallow } from 'zustand/react/shallow';

/**
 * Editor State Hook
 * 只返回编辑器的状态数据，不包含操作方法
 * 
 * 职责：
 * - 提供当前活动的 group、tab、nodes、variables 等状态
 * - 提供 events、functions、macros、dataframes 等集合
 * - 提供 UI 状态（contextMenu、selectedItemId 等）
 */
export function useEditorState() {
  // Get active IDs
  const activeGroupId = useLayoutStore((s: LayoutState) => s.activeGroupId) || 'default_editor';
  const activeEditorGroupId = useLayoutStore((s: LayoutState) => s.activeEditorGroupId) || 'default_editor';
  
  const activeEditorNode = useLayoutStore((s: LayoutState) => 
    s.activeEditorGroupId ? s.nodes[s.activeEditorGroupId] : null
  );
  
  const activeTabId = activeEditorNode?.data?.activeTabId || null;
  const tabs = activeEditorNode?.data?.tabs || [];
  const selectedNodeIds = activeEditorNode?.data?.params?.selectedNodeIds || [];

  // Get data
  const nodes = useTabNodes(activeTabId);
  const variables = useTabVariables(activeTabId);
  
  // Get the graphs object reference - only re-render when graphs object changes
  const graphs = useProjectStore((s) => s.graphs);
  const Variables = useProjectStore((s) => s.variables);
  const dataframes = useProjectStore((s) => s.databases);
  
  // Memoize the filtered collections - only recalculate when graphs changes
  const events = useMemo(() => {
    const result: Record<string, any> = {};
    for (const [id, graph] of Object.entries(graphs)) {
      if (graph.type === 'event') result[id] = graph;
    }
    return result;
  }, [graphs]);
  
  const functions = useMemo(() => {
    const result: Record<string, any> = {};
    for (const [id, graph] of Object.entries(graphs)) {
      if (graph.type === 'function') result[id] = graph;
    }
    return result;
  }, [graphs]);
  
  const macros = useMemo(() => {
    const result: Record<string, any> = {};
    for (const [id, graph] of Object.entries(graphs)) {
      if (graph.type === 'macro') result[id] = graph;
    }
    return result;
  }, [graphs]);

  // Get editor UI state
  const contextMenu = useEditorStore((s) => s.contextMenu);
  const selectedItemId = useEditorStore((s) => s.selectedItemId);
  const selectedItemType = useEditorStore((s) => s.selectedItemType);
  const pendingConnection = useEditorStore((s) => s.pendingConnection);

  // Get all groups (use shallow comparison to avoid unnecessary re-renders)
  const groupNodes = useLayoutStore(useShallow((s: LayoutState) =>
    Object.values(s.nodes).filter(n => n.type === 'component' && n.data?.tabs)
  ));

  const groups = useMemo(() => {
    return groupNodes.map(n => ({
      id: n.id,
      tabs: (n.data?.tabs || []).map(t => ({
        ...t,
        type: t.type || 'event'
      })) as any[],
      activeTabId: n.data?.activeTabId || null,
      selectedNodeIds: n.data?.params?.selectedNodeIds || []
    }));
  }, [groupNodes]);

  return useMemo(() => ({
    // Active IDs
    activeGroupId,
    activeEditorGroupId,
    activeTabId,
    
    // Current tab data
    tabs,
    nodes,
    variables,
    selectedNodeIds,
    
    // Global data
    Variables,
    
    // Collections
    events,
    functions,
    macros,
    dataframes,
    
    // All groups
    groups,
    
    // UI state
    contextMenu,
    selectedItemId,
    selectedItemType,
    pendingConnection,
  }), [
    activeGroupId,
    activeEditorGroupId,
    activeTabId,
    tabs,
    nodes,
    variables,
    selectedNodeIds,
    Variables,
    events,
    functions,
    macros,
    dataframes,
    groups,
    contextMenu,
    selectedItemId,
    selectedItemType,
    pendingConnection,
  ]);
}
