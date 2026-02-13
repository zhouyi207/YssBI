import { useCallback, useEffect, useRef, useMemo } from 'react';
import { useLayoutStore, LayoutState } from '@/features/layoutStore/layoutStore';
import { useNodeStore } from '@/features/node-registry/stores';
import { useProjectStore } from '@/features/project';
import { useViewportStore } from '@/features/canvas/stores';
import { useCanvasInteraction } from '@/features/canvas/hooks';
import { useEditorStore } from '../stores';
import { useEditorOperations } from './useEditorOperations';
import { useTabManagement } from './useTabManagement';
import { useProjectOperations } from './useProjectOperations';
import { useSubGraphManagement } from './useSubGraphManagement';
import { useVariableManagement } from './useVariableManagement';
import { useDataFrameManagement } from './useDataFrameManagement';
import { useTabNodes, useTabVariables } from '@/features/node-registry/stores/useNodeStore';
import { CanvasState } from '@/views/EditorView/Types/canvas';
import { BaseNode } from '@/views/EditorView/Types/nodes';
import { useShallow } from 'zustand/react/shallow';

const DEFAULT_VIEWPORT = { x: 0, y: 0, scale: 1 };

/**
 * Main Editor Hook
 * Combines all editor functionality into a single hook
 */
export function useEditor() {
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
  
  // Get collections
  const events = useProjectStore((s) => s.events);
  const functions = useProjectStore((s) => s.functions);
  const macros = useProjectStore((s) => s.macros);
  const globalVariables = useProjectStore((s) => s.globalVariables);
  const dataframes = useProjectStore((s) => s.dataframes);

  // Get editor state (use selectors to avoid unnecessary re-renders)
  const contextMenu = useEditorStore((s) => s.contextMenu);
  const setContextMenu = useEditorStore((s) => s.setContextMenu);
  const selectedItemId = useEditorStore((s) => s.selectedItemId);
  const selectedItemType = useEditorStore((s) => s.selectedItemType);
  const setSelectedInfo = useEditorStore((s) => s.setSelectedInfo);
  const pendingConnection = useEditorStore((s) => s.pendingConnection);
  const setPendingConnection = useEditorStore((s) => s.setPendingConnection);

  // Refs
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

  // Helper to switch sidebar tab
  const switchSidebarTab = useCallback((tab: 'events' | 'functions' | 'macros' | 'variables') => {
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

  // Setters
  const setNodes = useCallback((updater: BaseNode[] | ((prev: BaseNode[]) => BaseNode[])) => {
    const tId = activeTabIdRef.current;
    if (!tId) return;
    const currentNodes = useNodeStore.getState().getNodes(tId);
    const nextNodes = typeof updater === 'function' ? updater(currentNodes) : updater;
    useNodeStore.getState().setNodes(tId, nextNodes);
  }, []);

  const setCanvas = useCallback((updater: CanvasState | ((prev: CanvasState) => CanvasState), targetGroupId?: string) => {
    const gid = targetGroupId || activeGroupId;
    if (gid) useViewportStore.getState().setViewport(gid, updater);
  }, [activeGroupId]);

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

  const setActiveGroupId = useCallback((id: string) => {
    useLayoutStore.getState().setActiveGroup(id);
  }, []);

  // Get sub-hooks
  const editorOps = useEditorOperations();
  const tabMgmt = useTabManagement();
  const projectOps = useProjectOperations(tabMgmt.openSubGraph);
  const subGraphMgmt = useSubGraphManagement(tabMgmt.openSubGraph, tabMgmt.closeTab, switchSidebarTab);
  const variableMgmt = useVariableManagement(switchSidebarTab);
  const dataFrameMgmt = useDataFrameManagement();

  // Canvas interaction
  const canvasInteraction = useCanvasInteraction({
    activeGroupIdRef,
    activeTabIdRef,
    canvasRef,
    groups,
    setSelectedNodeIds,
    setNodes,
    setCanvas,
    saveHistory: editorOps.saveHistory
  });

  return useMemo(() => ({
    // State
    activeGroupId,
    activeEditorGroupId,
    activeTabId,
    tabs,
    nodes,
    variables,
    globalVariables,
    selectedNodeIds,
    groups,

    // Collections
    events,
    functions,
    macros,
    dataframes,

    // Editor state
    contextMenu,
    setContextMenu,
    selectedItemId,
    selectedItemType,
    setSelectedInfo,
    pendingConnection,
    setPendingConnection,

    // Setters
    setNodes,
    setCanvas,
    setSelectedNodeIds,
    setActiveGroupId,

    // Canvas interaction
    onCanvasWheel: canvasInteraction.onCanvasWheel,
    onCanvasPointerDown: canvasInteraction.onCanvasPointerDown,
    onNodePointerDown: canvasInteraction.onNodePointerDown,
    onPinPointerDown: canvasInteraction.onPinPointerDown,
    connectPins: canvasInteraction.connectPins,

    // Editor operations
    ...editorOps,

    // Tab management
    ...tabMgmt,

    // Project operations
    ...projectOps,

    // SubGraph management
    ...subGraphMgmt,

    // Variable management
    ...variableMgmt,

    // DataFrame management
    ...dataFrameMgmt,
  }), [
    activeGroupId,
    activeEditorGroupId,
    activeTabId,
    tabs,
    nodes,
    variables,
    globalVariables,
    selectedNodeIds,
    groups,
    events,
    functions,
    macros,
    dataframes,
    contextMenu,
    setContextMenu,
    selectedItemId,
    selectedItemType,
    setSelectedInfo,
    pendingConnection,
    setPendingConnection,
    setNodes,
    setCanvas,
    setSelectedNodeIds,
    setActiveGroupId,
    canvasInteraction.onCanvasWheel,
    canvasInteraction.onCanvasPointerDown,
    canvasInteraction.onNodePointerDown,
    canvasInteraction.onPinPointerDown,
    canvasInteraction.connectPins,
    editorOps,
    tabMgmt,
    projectOps,
    subGraphMgmt,
    variableMgmt,
    dataFrameMgmt,
  ]);
}
