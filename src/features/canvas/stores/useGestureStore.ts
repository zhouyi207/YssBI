import { create } from "zustand";
import { Pin } from "../Types/nodes";
import { Gesture } from "../Types/canvas";

interface GestureState {
    gesture: Gesture;
    setGesture: (gesture: Gesture) => void;

    // Helper for connection setup (optional, but keeps API similar)
    startConnection: (pin: Pin, startX: number, startY: number) => void;
    updateConnection: (x: number, y: number) => void;
    endConnection: () => void;
}

export const useGestureStore = create<GestureState>((set, get) => ({
    gesture: null,
    setGesture: (gesture) => set({ gesture }),

    startConnection: (pin, x, y) => set({
        gesture: {
            type: "connect",
            startPin: pin,
            startX: x,
            startY: y,
            currentX: x,
            currentY: y
        }
    }),

    updateConnection: (x, y) => {
        const current = get().gesture;
        if (current && current.type === "connect") {
            set({
                gesture: {
                    ...current,
                    currentX: x,
                    currentY: y
                }
            });
        }
    },

    endConnection: () => {
        const current = get().gesture;
        if (current && current.type === "connect") {
            set({ gesture: null });
        }
    },
}));
