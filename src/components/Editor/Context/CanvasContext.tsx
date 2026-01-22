import { createContext, useContext } from "react";
import { Pin, BaseNode } from "../Types/nodes";
import { CanvasState, Gesture, EditorGroup } from "../Types/canvas";


interface CanvasContextValue {
  canvas: CanvasState;
  setCanvas: (updater: CanvasState | ((prev: CanvasState) => CanvasState), targetGroupId?: string) => void;
  nodes: BaseNode[];
  setNodes: React.Dispatch<React.SetStateAction<BaseNode[]>>;
  onCanvasWheel: (e: React.WheelEvent, targetGroupId?: string) => void;
  onCanvasPointerDown: (e: React.PointerEvent, groupId?: string) => void;
  onNodePointerDown: (nodeId: string, e: React.PointerEvent, groupId?: string) => void;
  onPinPointerDown: (pinId: string, e: React.PointerEvent, groupId?: string) => void;
  selection: {
    startX: number;
    startY: number;
    currentX: number;
    currentY: number;
  } | null;

  gesture: Gesture;
  setGesture: (gesture: Gesture) => void;
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
  variables: Record<string, { name: string; type: string; value: any }>;
  globalVariables: Record<string, { name: string; type: string; value: any }>;
  selectedItemId: string | null;
  selectedItemType: 'variable' | 'event' | 'function' | 'macro' | 'setting' | null;
  setSelectedInfo: (id: string | null, type: 'variable' | 'event' | 'function' | 'macro' | 'setting' | null) => void;
  updateVariable: (id: string, data: Partial<{ name: string; type: string; value: any }>) => void;
  addVariable: (name: string, type: string, isGlobal?: boolean) => void;
  deleteVariable: (id: string) => void;
  promoteVariable: (id: string) => void;
  demoteVariable: (id: string) => void;
  functions: Record<string, import("../Types/canvas").SubGraphData>;
  addFunction: (name: string) => void;
  updateFunction: (id: string, data: Partial<import("../Types/canvas").SubGraphData>) => void;
  deleteFunction: (id: string) => void;
  events: Record<string, import("../Types/canvas").SubGraphData>;
  addEvent: (name: string) => void;
  updateEvent: (id: string, data: Partial<import("../Types/canvas").SubGraphData>) => void;
  deleteEvent: (id: string) => void;
  macros: Record<string, import("../Types/canvas").SubGraphData>;
  addMacro: (name: string) => void;
  updateMacro: (id: string, data: Partial<import("../Types/canvas").SubGraphData>) => void;
  deleteMacro: (id: string) => void;
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
  groups: EditorGroup[];
  activeGroupId: string;
  setActiveGroupId: (id: string) => void;
  splitEditorRight: (groupId: string) => void;
  closeGroup: (groupId: string) => void;

  // Tabs API (Now per-group)
  activeTabId: string | null;
  setActiveTabId: (id: string | null) => void;
  addTab: (title?: string) => void;
  openSubGraph: (id: string, name: string, type: "event" | "function" | "macro") => void;
  closeTab: (id: string, e?: React.MouseEvent) => void;
  openSettingsTab: () => void;
  pendingConnection: Pin | null;
  setPendingConnection: (pin: Pin | null) => void;
  tabNodes: Record<string, BaseNode[]>;
  tabVariables: Record<string, Record<string, { name: string; type: string; value: any }>>;
}

export const CanvasContext = createContext<CanvasContextValue | null>(null);
export const GroupContext = createContext<string | null>(null);

export function useCanvas() {
  const ctx = useContext(CanvasContext);
  if (!ctx) {
    throw new Error("useCanvas must be used within CanvasProvider");
  }

  const currentGroupId = useContext(GroupContext);
  // If we are in a specific group context, use that ID. Otherwise fallback to the globally active one.
  const activeGroupId = currentGroupId || ctx.activeGroupId;

  // Resolve the group object for this context
  const group = ctx.groups.find(g => g.id === activeGroupId) || ctx.groups[0];

  // Resolve the data specifically for this group's active tab
  const activeTabId = group.activeTabId;
  const nodes = activeTabId ? ctx.tabNodes[activeTabId] || [] : [];
  const variables = activeTabId ? ctx.tabVariables[activeTabId] || {} : {};
  const canvas = group.canvas;

  // Helper to activate this group when interaction starts
  const ensureActive = () => {
    if (ctx.activeGroupId !== activeGroupId) {
      ctx.setActiveGroupId(activeGroupId);
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
    tabs: group.tabs,
    activeTabId,
    // Override global state with localized state
    nodes,
    canvas,
    variables,
    // Override handlers
    onCanvasPointerDown: wrappedOnCanvasPointerDown,
    onNodePointerDown: wrappedOnNodePointerDown,
    onPinPointerDown: wrappedOnPinPointerDown,
    onCanvasWheel: wrappedOnCanvasWheel,
    setCanvas: wrappedSetCanvas,
  };
}