import { createContext, useContext } from "react";
import { Pin, BaseNode } from "../node/models";
import { CanvasState, Gesture, Tab } from "./type";

interface CanvasContextValue {
  canvas: CanvasState;
  setCanvas: (canvas: CanvasState) => void;
  nodes: BaseNode[];
  setNodes: React.Dispatch<React.SetStateAction<BaseNode[]>>;
  onCanvasWheel: (e: React.WheelEvent) => void;
  onCanvasPointerDown: (e: React.PointerEvent) => void;
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
  selectedVariableId: string | null;
  setSelectedVariableId: (id: string | null) => void;
  updateVariable: (id: string, data: Partial<{ name: string; type: string; value: any }>) => void;
  addVariable: (name: string, type: string, isGlobal?: boolean) => void;
  deleteVariable: (id: string) => void;
  promoteVariable: (id: string) => void;
  demoteVariable: (id: string) => void;
  functions: Record<string, { name: string }>;
  addFunction: (name: string) => void;
  deleteFunction: (id: string) => void;
  macros: Record<string, { name: string }>;
  addMacro: (name: string) => void;
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
  setActiveTabId: (id: string) => void;
  addTab: (title?: string) => void;
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