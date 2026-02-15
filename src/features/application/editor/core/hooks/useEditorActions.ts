import { useCallback, useRef, useEffect } from 'react';
import { useLayoutStore, LayoutState } from '@/features/core/layout/layoutStore';
import { useProjectStore } from '@/features/core/project';
import { useViewportStore } from '@/features/domain/canvas/stores';
import { useEditorStore } from '../stores';
import { GraphPosition} from '@/shared/types/domain';
import { Node } from '@/shared/types/ui';
import { DEFAULT_VIEWPORT } from '@/app/appConfig/default';
import { deserializeGraph } from '@/shared/utils/editor';


/**
 * Editor Actions Hook
 * 只返回编辑器的操作方法，不包含状态
 * 
 * 职责：
 * - 提供基础的 setter 方法（setNodes, setCanvas, setSelectedNodeIds 等）
 * - 提供 UI 状态的 setter（setContextMenu, setSelectedInfo, setPendingConnection）
 */
export function useEditorActions() {
  const activeGroupId = useLayoutStore((s: LayoutState) => s.activeGroupId) || 'default_editor';
  const activeEditorNode = useLayoutStore((s: LayoutState) => 
    s.activeEditorGroupId ? s.nodes[s.activeEditorGroupId] : null
  );
  const activeTabId = activeEditorNode?.data?.activeTabId || null;

  // Refs for stable access
  const activeGroupIdRef = useRef(activeGroupId);
  const activeTabIdRef = useRef(activeTabId);
  const canvasRef = useRef(useViewportStore.getState().viewports[activeGroupId] || DEFAULT_VIEWPORT);

  // Update refs
  activeGroupIdRef.current = activeGroupId;
  activeTabIdRef.current = activeTabId;

  // Update canvas ref when viewport changes
  useEffect(() => {
    const unsub = useViewportStore.subscribe((state) => {
      const currentGroupId = useLayoutStore.getState().activeGroupId;
      if (currentGroupId && state.viewports[currentGroupId]) {
        canvasRef.current = state.viewports[currentGroupId];
      }
    });
    const current = useViewportStore.getState().viewports[useLayoutStore.getState().activeGroupId || ''];
    if (current) canvasRef.current = current;
    return unsub;
  }, []);

  // Get UI state setters
  const setContextMenu = useEditorStore((s) => s.setContextMenu);
  const setSelectedInfo = useEditorStore((s) => s.setSelectedInfo);
  const setPendingConnection = useEditorStore((s) => s.setPendingConnection);

  // Node operations
  const setNodes = useCallback((updater: Node[] | ((prev: Node[]) => Node[])) => {
    const tId = activeTabIdRef.current;
    if (!tId) return;
    const graphData = useProjectStore.getState().graphs[tId];
    if (!graphData) return;
    
    const { nodes: currentNodes } = deserializeGraph(graphData);
    const nextNodes = typeof updater === 'function' ? updater(currentNodes) : updater;
    
    // Update the graph in project store
    useProjectStore.getState().updateGraph(tId, { nodes: nextNodes as any });
  }, []);

  // Canvas operations
  const setCanvas = useCallback((updater: GraphPosition | ((prev: GraphPosition) => GraphPosition), targetGroupId?: string) => {
    const gid = targetGroupId || activeGroupId;
    if (gid) useViewportStore.getState().setViewport(gid, updater);
  }, [activeGroupId]);

  // Selection operations
  const setSelectedNodeIds = useCallback((updater: string[] | ((prev: string[]) => string[]), targetGroupId?: string) => {
    const gid = targetGroupId || activeGroupId;
    if (gid) {
      const state = useLayoutStore.getState() as LayoutState;
      const node = state.nodes[gid];
      if (node) {
        const current = node.data?.params?.selectedNodeIds || [];
        const next = typeof updater === 'function' ? updater(current) : updater;
        useLayoutStore.getState().updateNode(gid, {
          data: {
            ...node.data,
            params: { ...node.data?.params, selectedNodeIds: next }
          }
        });
      }
    }
  }, [activeGroupId]);

  // Group operations
  const setActiveGroupId = useCallback((id: string) => {
    useLayoutStore.getState().setActiveGroup(id);
  }, []);

  // Helper to switch sidebar tab
  const switchSidebarTab = useCallback((tab: 'events' | 'functions' | 'macros' | 'variables' | 'data') => {
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

  return {
    // Refs (for stable access in callbacks)
    activeGroupIdRef,
    activeTabIdRef,
    canvasRef,
    
    // Node operations
    setNodes,
    
    // Canvas operations
    setCanvas,
    
    // Selection operations
    setSelectedNodeIds,
    
    // Group operations
    setActiveGroupId,
    
    // UI state setters
    setContextMenu,
    setSelectedInfo,
    setPendingConnection,
    
    // Helper
    switchSidebarTab,
  };
}
