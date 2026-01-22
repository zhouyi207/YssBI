import { create } from "zustand";

interface SelectionState {
    startX: number;
    startY: number;
    currentX: number;
    currentY: number;
    isVisible: boolean;

    startSelection: (x: number, y: number) => void;
    updateSelection: (x: number, y: number) => void;
    endSelection: () => void;
}

export const useSelectionStore = create<SelectionState>((set) => ({
    startX: 0,
    startY: 0,
    currentX: 0,
    currentY: 0,
    isVisible: false,

    startSelection: (x, y) => set({ startX: x, startY: y, currentX: x, currentY: y, isVisible: true }),
    updateSelection: (x, y) => set({ currentX: x, currentY: y }),
    endSelection: () => set({ isVisible: false }),
}));
