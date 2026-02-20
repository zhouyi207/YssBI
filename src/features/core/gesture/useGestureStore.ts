import { create } from "zustand";
import { Pin } from "@/shared/types/domain";
import { EditorGesture } from "@/shared/types/ui";

interface GestureState {
    gesture: EditorGesture;
    setGesture: (gesture: EditorGesture) => void;
    /** 有实际移动时，抑制下一次 contextmenu（避免拖拽松手误触） */
    suppressNextContextMenu: boolean;
    clearGesture: (hadMovement?: boolean) => void;
    consumeSuppressContextMenu: () => boolean;

    // Helper for connection setup (optional, but keeps API similar)
    startConnection: (pin: Pin, startX: number, startY: number) => void;
    updateConnection: (x: number, y: number) => void;
    endConnection: () => void;
}

export const useGestureStore = create<GestureState>((set, get) => ({
    gesture: null,
    suppressNextContextMenu: false,
    setGesture: (gesture) => set({ gesture }),
    clearGesture: (hadMovement = false) => set({
        gesture: null,
        suppressNextContextMenu: hadMovement,
    }),
    consumeSuppressContextMenu: () => {
        const v = get().suppressNextContextMenu;
        if (v) set({ suppressNextContextMenu: false });
        return v;
    },

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
