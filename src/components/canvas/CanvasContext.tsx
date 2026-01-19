import { createContext, useContext } from "react";
import { Pin, BaseNode } from "../node/models";
import { CanvasState, Gesture } from "./type";

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
  exportGraph: () => Promise<void>;
  saveGraph: () => Promise<void>;
  importGraph: (json?: string) => Promise<void>;
  executeGraph: () => Promise<void>;
  variables: Record<string, { name: string; type: string; value: any }>;
  selectedVariableId: string | null;
  setSelectedVariableId: (id: string | null) => void;
  updateVariable: (id: string, data: Partial<{ name: string; type: string; value: any }>) => void;
  addVariable: (name: string, type: string) => void;
  deleteVariable: (id: string) => void;
}

export const CanvasContext = createContext<CanvasContextValue | null>(null);

export function useCanvas() {
  const ctx = useContext(CanvasContext);
  if (!ctx) {
    throw new Error("useCanvas must be used within CanvasProvider");
  }
  return ctx;
}