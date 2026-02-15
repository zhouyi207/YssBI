import { useMemo } from 'react';
import { useCanvasInteraction } from '@/features/domain/canvas/hooks';
import { useEditorState } from './useEditorState';
import { useEditorActions } from './useEditorActions';
import { useEditorOperations } from './useEditorOperations';
import { useTabManagement } from './useTabManagement';
import { useProjectOperations } from './useProjectOperations';
import { useGraphManagement } from './useGraphManagement';
import { useVariableManagement } from './useVariableManagement';
import { useDataFrameManagement } from './useDataFrameManagement';

/**
 * Main Editor Hook (Refactored)
 * 
 * 组合所有编辑器功能的主 hook
 * 
 * 重构说明：
 * - 拆分为 useEditorState（状态）和 useEditorActions（操作）
 * - 减少单个 hook 的复杂度
 * - 提高可测试性和可维护性
 * 
 * 使用方式：
 * ```tsx
 * // 完整功能（向后兼容）
 * const editor = useEditor();
 * 
 * // 只需要状态
 * const state = useEditorState();
 * 
 * // 只需要操作
 * const actions = useEditorActions();
 * ```
 */
export function useEditor() {
  // Get state
  const state = useEditorState();
  
  // Get actions
  const actions = useEditorActions();

  // Get sub-hooks
  const editorOps = useEditorOperations();
  const tabMgmt = useTabManagement();
  const projectOps = useProjectOperations(tabMgmt.openSubGraph);
  const subGraphMgmt = useGraphManagement(tabMgmt.openSubGraph, tabMgmt.closeTab, actions.switchSidebarTab);
  const variableMgmt = useVariableManagement(actions.switchSidebarTab);
  const dataFrameMgmt = useDataFrameManagement(actions.switchSidebarTab);

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

    // SubGraph management
    ...subGraphMgmt,

    // Variable management
    ...variableMgmt,

    // DataFrame management
    ...dataFrameMgmt,
  }), [
    state,
    actions,
    canvasInteraction,
    editorOps,
    tabMgmt,
    projectOps,
    subGraphMgmt,
    variableMgmt,
    dataFrameMgmt,
  ]);
}
