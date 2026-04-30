import { create } from "zustand";
import type { GraphResourceDragData } from "@/features/core/dnd";

export interface SidebarDragState {
  type: string;
  template: any;
  sidebarResource?: GraphResourceDragData;
  x: number;
  y: number;
}

interface SidebarDragStore {
  activeDrag: SidebarDragState | null;
  setActiveDrag: (drag: SidebarDragState | null) => void;
  updatePosition: (x: number, y: number) => void;
}

export const useSidebarDragStore = create<SidebarDragStore>((set) => ({
  activeDrag: null,
  setActiveDrag: (drag) => set({ activeDrag: drag }),
  updatePosition: (x, y) =>
    set((state) =>
      state.activeDrag
        ? { activeDrag: { ...state.activeDrag, x, y } }
        : state
    ),
}));
