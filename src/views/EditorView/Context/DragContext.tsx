import { createContext, useContext } from "react";
import { DragState } from "../Types/drag";

interface DragContextValue {
  drag: DragState;
  startDrag: (drag: DragState) => void;
  updatePosition: (x: number, y: number) => void;
  endDrag: () => void;
}

export const DragContext = createContext<DragContextValue | null>(null);

export function useDrag() {
  const ctx = useContext(DragContext);
  if (!ctx) {
    throw new Error("useDrag must be used within DragProvider");
  }
  return ctx;
}
