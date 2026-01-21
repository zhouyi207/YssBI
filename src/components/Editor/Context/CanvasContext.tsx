import { createContext, useContext } from "react";
import { Pin, BaseNode } from "../Types/nodes";
import { CanvasState, Gesture, Tab } from "../Types/canvas";

interface CanvasContextValue {
  canvas: CanvasState;
  setCanvas: (canvas: CanvasState) => void;
  nodes: BaseNode[];
  setNodes: React.Dispatch<React.SetStateAction<BaseNode[]>>;
  onCanvasWheel: (e: React.WheelEvent) => void;
  onCanvasPointerDown: (e: React.PointerEvent) => void;
  onNodePointerDown: (e: React.PointerEvent, node: BaseNode) => void;
  onPinPointerDown: (e: React.PointerEvent, pin: Pin) => void;
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
  selectedItemType: 'variable' | 'event' | 'function' | 'macro' | null;
  setSelectedInfo: (id: string | null, type: 'variable' | 'event' | 'function' | 'macro' | null) => void;
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
  // Tabs API
  tabs: Tab[];
  activeTabId: string | null;
  setActiveTabId: (id: string | null) => void;
  addTab: (title?: string) => void;
  openSubGraph: (id: string, name: string, type: "event" | "function" | "macro") => void;
  closeTab: (id: string, e?: React.MouseEvent) => void;
  pendingConnection: Pin | null;
  setPendingConnection: (pin: Pin | null) => void;
}

export const CanvasContext = createContext<CanvasContextValue | null>(null);

export function useCanvas() {
  const ctx = useContext(CanvasContext);
  if (!ctx) {
    throw new Error("useCanvas must be used within CanvasProvider");
  }
  return ctx;
}