import { createContext, useContext } from "react";
import { CanvasState } from "./type";

interface CanvasContextValue {
  canvas: CanvasState;
  setCanvas: (canvas: CanvasState) => void;
  onCanvasWheel: (e: React.WheelEvent) => void;
  onCanvasPointerDown: (e: React.PointerEvent) => void;
  selection: {
    startX: number;
    startY: number;
    currentX: number;
    currentY: number;
  } | null;
  contextMenu: {
    x: number;
    y: number;
    visible: boolean;
  } | null;
  setContextMenu: (menu: { x: number; y: number; visible: boolean } | null) => void;
  variableDropMenu: {
    x: number;
    y: number;
    worldX: number;
    worldY: number;
    varType: string;
  } | null;
  setVariableDropMenu: (menu: { x: number; y: number; worldX: number; worldY: number; varType: string } | null) => void;
}

export const CanvasContext = createContext<CanvasContextValue | null>(null);

export function useCanvas() {
  const ctx = useContext(CanvasContext);
  if (!ctx) {
    throw new Error("useCanvas must be used within CanvasProvider");
  }
  return ctx;
}