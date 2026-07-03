import { useMemo } from 'react';
import { useCanvasInteraction } from '@/features/core/canvas';
import { useEditorState, useEditorActions } from '@/features/core/editor';
import { useEditorOperations } from './useEditorOperations';
import { useTabManagement } from './useTabManagement';
import { useOpenWorksheet, useWorksheetManagement } from './useWorksheetManagement';
import { useProjectOperations } from './useProjectOperations';
import { useGraphManagement, useVariableManagement, useDatabaseManagement, useNodeManagement } from '@/features/application/dataManagement';

export function useEditor(options?: { withCanvasInteraction?: boolean }) {
  const withCanvasInteraction = options?.withCanvasInteraction ?? true;
  // Get state
  const state = useEditorState();
  
  // Get actions
  const actions = useEditorActions();

  // Get sub-hooks
  const editorOps = useEditorOperations();
  const tabMgmt = useTabManagement();
  const openWorksheet = useOpenWorksheet();
  const worksheetMgmt = useWorksheetManagement(openWorksheet);
  const projectOps = useProjectOperations(tabMgmt.openGraph);
  
  const graphMgmt = useGraphManagement(tabMgmt.openGraph);
  const variableMgmt = useVariableManagement();
  const dataFrameMgmt = useDatabaseManagement();
  const nodeMgmt = useNodeManagement();

  // Canvas interaction（Sidebar 等组件禁用，避免重复全局监听）
  const canvasInteraction = useCanvasInteraction({
    activeGroupIdRef: actions.activeGroupIdRef,
    activeTabIdRef: actions.activeTabIdRef,
    canvasRef: actions.canvasRef,
    groups: state.groups,
    setSelectedNodeIds: actions.setSelectedNodeIds,
    setNodes: actions.setNodes,
    enabled: withCanvasInteraction,
  });

  return useMemo(() => ({
    // State (from useEditorState)
    ...state,

    // Actions (from useEditorActions)
    setNodes: actions.setNodes,
    setCanvas: actions.setCanvas,
    setSelectedNodeIds: actions.setSelectedNodeIds,
    setActiveGroupId: actions.setActiveGroupId,
    setContextMenu: actions.setContextMenu,
    setDetailFocus: actions.setDetailFocus,
    clearDetailFocus: actions.clearDetailFocus,
    setPendingConnection: actions.setPendingConnection,

    // Canvas interaction
    onCanvasPointerDown: canvasInteraction.onCanvasPointerDown,
    onNodePointerDown: canvasInteraction.onNodePointerDown,
    onPinPointerDown: canvasInteraction.onPinPointerDown,
    connectPins: canvasInteraction.connectPins,

    // Editor operations
    ...editorOps,

    // Tab management
    ...tabMgmt,
    openWorksheet,

    // Worksheet management
    ...worksheetMgmt,

    // Project operations
    ...projectOps,

    // Graph management
    ...graphMgmt,

    // Variable management
    ...variableMgmt,

    // DataFrame management
    ...dataFrameMgmt,

    // Node management (override graph management's node handlers)
    createNode: nodeMgmt.createNode,
    createNodes: nodeMgmt.createNodes,
    deleteNode: nodeMgmt.deleteNode,
    deleteNodes: nodeMgmt.deleteNodes,
    handleNodeCreated: nodeMgmt.handleNodeCreated,
    handleNodeDeleted: nodeMgmt.handleNodeDeleted,
  }), [
    state,
    actions,
    canvasInteraction,
    editorOps,
    tabMgmt,
    openWorksheet,
    worksheetMgmt,
    projectOps,
    graphMgmt,
    variableMgmt,
    dataFrameMgmt,
    nodeMgmt,
  ]);
}
