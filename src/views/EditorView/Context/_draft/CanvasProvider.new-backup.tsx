import React, { useMemo, useCallback } from "react";
import { CanvasContext } from "@/views/EditorView/Context/CanvasContext";
import { useEditor } from "@/features/editor/hooks/useEditor";
import { useEditorKeyboard } from "@/features/editor/hooks/useEditorKeyboard";
import { useViewportStore } from "@/features/canvas/stores";
import { useLayoutStore } from "@/features/layoutStore/layoutStore";

const DEFAULT_VIEWPORT = { x: 0, y: 0, scale: 1 };

export const CanvasProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  // Get all editor functionality from the main hook
  const editor = useEditor();

  // Helper to get active canvas local point
  const getActiveCanvasLocalPoint = useCallback((clientX: number, clientY: number) => {
    const gid = useLayoutStore.getState().activeEditorGroupId || useLayoutStore.getState().activeGroupId || 'default_editor';
    const el = document.getElementById(`layout-node-${gid}`);
    if (!el) return { x: 0, y: 0 };
    const rect = el.getBoundingClientRect();
    const currentCanvas = useViewportStore.getState().viewports[gid] || DEFAULT_VIEWPORT;
    return {
      x: (clientX - rect.left - currentCanvas.x) / currentCanvas.scale,
      y: (clientY - rect.top - currentCanvas.y) / currentCanvas.scale
    };
  }, []);

  // Setup keyboard shortcuts
  useEditorKeyboard({
    deleteSelected: editor.deleteSelected,
    undo: editor.undo,
    redo: editor.redo,
    copy: editor.copy,
    cut: editor.cut,
    paste: editor.paste,
    saveGraph: editor.saveGraph,
    saveGraphAs: editor.saveGraphAs,
    importGraph: editor.importGraph,
    addEvent: editor.addEvent,
    closeTab: editor.closeTab,
    setActiveTabId: editor.setActiveTabId,
    splitEditorRight: editor.splitEditorRight,
    getActiveCanvasLocalPoint,
  });

  // Build context value
  const contextValue = useMemo(() => ({
    // Canvas operations
    setCanvas: editor.setCanvas,
    nodes: [],
    setNodes: editor.setNodes,
    onCanvasWheel: editor.onCanvasWheel,
    onCanvasPointerDown: editor.onCanvasPointerDown,
    onNodePointerDown: editor.onNodePointerDown,
    onPinPointerDown: editor.onPinPointerDown,
    
    // Context menu
    contextMenu: editor.contextMenu,
    setContextMenu: editor.setContextMenu,
    
    // Project operations
    saveGraphAs: editor.saveGraphAs,
    saveGraph: editor.saveGraph,
    importGraph: editor.importGraph,
    executeGraph: editor.executeGraph,
    executeAllEvents: editor.executeAllEvents,
    
    // Variables
    variables: editor.variables,
    globalVariables: editor.globalVariables,
    addVariable: editor.addVariable,
    updateVariable: editor.updateVariable,
    deleteVariable: editor.deleteVariable,
    promoteVariable: editor.promoteVariable,
    demoteVariable: editor.demoteVariable,
    
    // Selection
    selectedItemId: editor.selectedItemId,
    selectedItemType: editor.selectedItemType,
    setSelectedInfo: editor.setSelectedInfo,
    
    // Events
    events: editor.events,
    addEvent: editor.addEvent,
    updateEvent: editor.updateEvent,
    deleteEvent: editor.deleteEvent,
    
    // Functions
    functions: editor.functions,
    addFunction: editor.addFunction,
    updateFunction: editor.updateFunction,
    deleteFunction: editor.deleteFunction,
    
    // Macros
    macros: editor.macros,
    addMacro: editor.addMacro,
    updateMacro: editor.updateMacro,
    deleteMacro: editor.deleteMacro,
    
    // DataFrames
    dataframes: editor.dataframes,
    addDataFrame: editor.addDataFrame,
    updateDataFrame: editor.updateDataFrame,
    deleteDataFrame: editor.deleteDataFrame,
    
    // History
    undo: editor.undo,
    redo: editor.redo,
    canUndo: editor.canUndo,
    canRedo: editor.canRedo,
    saveHistory: editor.saveHistory,
    
    // Clipboard
    copy: editor.copy,
    paste: editor.paste,
    cut: editor.cut,
    deleteSelected: editor.deleteSelected,
    
    // Connections
    connectPins: editor.connectPins,
    
    // Groups
    groups: editor.groups,
    activeGroupId: editor.activeGroupId,
    activeEditorGroupId: editor.activeEditorGroupId,
    setActiveGroupId: editor.setActiveGroupId,
    splitEditorRight: editor.splitEditorRight,
    closeGroup: editor.closeGroup,
    
    // Tabs
    activeTabId: editor.activeTabId,
    setActiveTabId: editor.setActiveTabId,
    openSubGraph: editor.openSubGraph,
    closeTab: editor.closeTab,
    openSettingsTab: editor.openSettingsTab,
    
    // Pending connection
    pendingConnection: editor.pendingConnection,
    setPendingConnection: editor.setPendingConnection,
    
    // Selected nodes
    selectedNodeIds: editor.selectedNodeIds,
    setSelectedNodeIds: editor.setSelectedNodeIds,
  }), [editor]);

  return (
    <CanvasContext.Provider value={contextValue}>
      {children}
    </CanvasContext.Provider>
  );
};
