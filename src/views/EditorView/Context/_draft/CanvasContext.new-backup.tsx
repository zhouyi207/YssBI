import { createContext, useContext, useCallback } from "react";
import { Pin, BaseNode } from "../Types/nodes";
import { CanvasState } from "../Types/canvas";
import { VariableDefinition } from "../Types/variables";
import { useTabNodes, useTabVariables } from "@/features/node-registry/stores/useNodeStore";
import { useLayoutStore, LayoutState } from "../../../features/layoutStore/layoutStore";
import { useShallow } from 'zustand/react/shallow';

interface CanvasContextValue {
  setCanvas: (updater: CanvasState | ((prev: CanvasState) => CanvasState), targetGroupId?: string) => void;
  nodes: BaseNode[];
  setNodes: React.Dispatch<React.SetStateAction<BaseNode[]>>;
  onCanvasWheel: (e: React.WheelEvent, targetGroupId?: string) => void;
  onCanvasPointerDown: (e: React.PointerEvent, groupId?: string) => void;
  onNodePointerDown: (nodeId: string, e: React.PointerEvent, groupId?: string) => void;
  onPinPointerDown: (pinId: string, e: React.PointerEvent, groupId?: string) => void;
  contextMenu: {
    x: number;
    y: number;
    visible: boolean;
  } | null;
  setContextMenu: (menu: { x: number; y: number; visible: boolean } | null) => void;
  saveGraphAs: () => Promise<void>;
  saveGraph: () => Promise<void>;
  importGraph: (json?: string) => Promise<void>;
  executeGraph: () => Promise<void>;
  executeAllEvents: () => Promise<void>;
  variables: Record<string, VariableDefinition>;
  globalVariables: Record<string, VariableDefinition>;
  selectedItemId: string | null;
  selectedItemType: 'variable' | 'event' | 'function' | 'macro' | 'data' | 'setting' | null;
  setSelectedInfo: (id: string | null, type: 'variable' | 'event' | 'function' | 'macro' | 'data' | 'setting' | null) => void;
  updateVariable: (id: string, data: Partial<VariableDefinition>) => void;
  addVariable: (name?: string, type?: string, isGlobal?: boolean) => void;
  deleteVariable: (id: string) => void;
  promoteVariable: (id: string) => void;
  demoteVariable: (id: string) => void;
  functions: Record<string, import("../Types/canvas").SubGraphData>;
  addFunction: (name?: string) => void;
  updateFunction: (id: string, data: Partial<import("../Types/canvas").SubGraphData>) => void;
  deleteFunction: (id: string) => void;
  events: Record<string, import("../Types/canvas").SubGraphData>;
  addEvent: (name?: string) => void;
  updateEvent: (id: string, data: Partial<import("../Types/canvas").SubGraphData>) => void;
  deleteEvent: (id: string) => void;
  macros: Record<string, import("../Types/canvas").SubGraphData>;
  addMacro: (name?: string) => void;
  updateMacro: (id: string, data: Partial<import("../Types/canvas").SubGraphData>) => void;
  deleteMacro: (id: string) => void;
  dataframes: Record<string, import("../Types/canvas").DataFrameData>;
  addDataFrame: (name?: string) => void;
  updateDataFrame: (id: string, data: Partial<import("../Types/canvas").DataFrameData>) => void;
  deleteDataFrame: (id: string) => void;
  undo: () => void;
  redo: () => void;
  copy: () => void;
  paste: (pos?: { x: number; y: number }) => void;
  cut: () => void;
  deleteSelected: () => void;
  canUndo: boolean;
  canRedo: boolean;
  saveHistory: () => void;
  connectPins: (pinAId: string, pinBId: string) => void;
  // Groups API
  groups: any[];
  activeGroupId: string;
  activeEditorGroupId: string;
  setActiveGroupId: (id: string) => void;
  splitEditorRight: (groupId: string) => void;
  closeGroup: (groupId: string) => void;

  // Tabs API (Now per-group)
  activeTabId: string | null;
  setActiveTabId: (id: string | null) => void;
  openSubGraph: (id: string, name: string, type: "event" | "function" | "macro") => void;
  closeTab: (id: string, e?: React.MouseEvent) => void;
  openSettingsTab: () => void;
  pendingConnection: Pin | null;
  setPendingConnection: (pin: Pin | null) => void;

  selectedNodeIds: string[];
  setSelectedNodeIds: (updater: string[] | ((prev: string[]) => string[]), targetGroupId?: string) => void;
}

export const CanvasContext = createContext<CanvasContextValue | null>(null);
export const GroupContext = createContext<string | null>(null);

/**
 * useCanvas Hook
 * 
 * Provides context-aware access to canvas operations.
 * When used within a GroupContext, it automatically scopes operations to that group.
 * Otherwise, it uses the globally active group.
 */
export function useCanvas() {
  const ctx = useContext(CanvasContext);
  if (!ctx) {
    throw new Error("useCanvas must be used within CanvasProvider");
  }

  const currentGroupId = useContext(GroupContext);
  const activeGroupIdFromStore = useLayoutStore(useCallback((s: LayoutState) => s.activeGroupId, []));
  
  // If we are in a specific group context, use that ID. Otherwise fallback to the globally active one.
  const activeGroupId = currentGroupId || activeGroupIdFromStore || 'default_editor';

  // Resolve the group object for this context from layoutStore
  const nodeSelector = useCallback((s: LayoutState) => s.nodes[activeGroupId], [activeGroupId]);
  const node = useLayoutStore(useShallow(nodeSelector));
  
  // Core logic: If current Context points to a non-editor node (like Sidebar/Detail),
  // data logic (tabs/nodes/variables) should fall back to the currently active editor group.
  const activeEditorGroupId = useLayoutStore((s: LayoutState) => s.activeEditorGroupId);
  const isEditor = node?.type === 'component' && !!node.data?.tabs;
  
  const functionalNode = isEditor ? node : (useLayoutStore.getState().nodes[activeEditorGroupId || ''] || node);

  const tabs = functionalNode?.data?.tabs || [];
  const activeTabId = functionalNode?.data?.activeTabId || null;

  // Use the custom hook to efficiently retrieve nodes for the active tab
  const nodes = useTabNodes(activeTabId);
  const variables = useTabVariables(activeTabId);
  const selectedNodeIds = functionalNode?.data?.params?.selectedNodeIds || [];

  // Helper to activate this group when interaction starts
  const setActiveGroup = useLayoutStore(s => s.setActiveGroup);
  const ensureActive = () => {
    if (activeGroupIdFromStore !== activeGroupId) {
      setActiveGroup(activeGroupId);
    }
  };

  // Wrap interaction handlers to ensure the correct group is active
  const wrappedOnCanvasPointerDown = (e: React.PointerEvent) => {
    ensureActive();
    ctx.onCanvasPointerDown(e, activeGroupId);
  };

  const wrappedOnNodePointerDown = (nodeId: string, e: React.PointerEvent) => {
    ensureActive();
    ctx.onNodePointerDown(nodeId, e, activeGroupId);
  };

  const wrappedOnPinPointerDown = (pinId: string, e: React.PointerEvent) => {
    ensureActive();
    ctx.onPinPointerDown(pinId, e, activeGroupId);
  };

  const wrappedOnCanvasWheel = (e: React.WheelEvent, targetGroupId?: string) => {
    ensureActive();
    ctx.onCanvasWheel(e, targetGroupId || activeGroupId);
  };

  const wrappedSetCanvas = (updater: CanvasState | ((prev: CanvasState) => CanvasState), targetGroupId?: string) => {
    ensureActive();
    ctx.setCanvas(updater, targetGroupId || activeGroupId);
  };

  // Merge global context with group-specific state
  return {
    ...ctx,
    groupId: activeGroupId,
    tabs,
    activeTabId,
    // Override global state with localized state
    nodes,
    variables,
    selectedNodeIds,
    // Override handlers
    onCanvasPointerDown: wrappedOnCanvasPointerDown,
    onNodePointerDown: wrappedOnNodePointerDown,
    onPinPointerDown: wrappedOnPinPointerDown,
    onCanvasWheel: wrappedOnCanvasWheel,
    setCanvas: wrappedSetCanvas,
  };
}
