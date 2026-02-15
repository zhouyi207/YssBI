import { useMemo } from 'react';
import { useCanvasInteraction } from '@/features/domain/canvas/hooks';
import { useEditorState } from './useEditorState';
import { useEditorActions } from './useEditorActions';
import { useEditorOperations } from './useEditorOperations';
import { useTabManagement } from './useTabManagement';
import { useProjectOperations } from './useProjectOperations';
import { useGraphManagement } from './useGraphManagement';
import { useVariableManagement } from './useVariableManagement';
import { useDatabaseManagement } from './useDatabaseManagement';
import { useNodeManagement } from './useNodeManagement';


export function useEditor() {
  // Get state
  const state = useEditorState();
  
  // Get actions
  const actions = useEditorActions();

  // Get sub-hooks
  const editorOps = useEditorOperations();
  const tabMgmt = useTabManagement();
  const projectOps = useProjectOperations(tabMgmt.openGraph);
  
  const graphMgmt = useGraphManagement(tabMgmt.openGraph, tabMgmt.closeTab);
  const variableMgmt = useVariableManagement();
  const dataFrameMgmt = useDatabaseManagement();
  const nodeMgmt = useNodeManagement();

  // Canvas interaction
  const canvasInteraction = useCanvasInteraction({
    activeGroupIdRef: actions.activeGroupIdRef,
    activeTabIdRef: actions.activeTabIdRef,
    canvasRef: actions.canvasRef,
    groups: state.groups,
    setSelectedNodeIds: actions.setSelectedNodeIds,
    setNodes: actions.setNodes,
    setCanvas: actions.setCanvas,
    saveHistory: editorOps.saveHistory
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
    setSelectedInfo: actions.setSelectedInfo,
    setPendingConnection: actions.setPendingConnection,

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
    projectOps,
    graphMgmt,
    variableMgmt,
    dataFrameMgmt,
    nodeMgmt,
  ]);
}
