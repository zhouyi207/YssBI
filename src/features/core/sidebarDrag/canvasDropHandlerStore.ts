import { createStore } from "zustand/vanilla";
import type { SidebarDragState } from "@/features/core/dnd";

export type CanvasDropHandler = (dragState: SidebarDragState) => void | Promise<boolean>;

interface CanvasDropHandlerState {
  handlers: Record<string, { readonly handler: CanvasDropHandler } | undefined>;
}

const dropHandlerStore = createStore<CanvasDropHandlerState>(() => ({
  handlers: {},
}));

export const canvasDropHandlerStore = {
  registerHandler: (panelInstanceId: string, handler: CanvasDropHandler): (() => void) => {
    // Callback identity may be reused by successive mounts of the same panel.
    const registration = { handler };
    dropHandlerStore.setState((state) => ({
      handlers: { ...state.handlers, [panelInstanceId]: registration },
    }));
    return () => {
      dropHandlerStore.setState((state) => {
        if (state.handlers[panelInstanceId] !== registration) return state;
        const handlers = { ...state.handlers };
        delete handlers[panelInstanceId];
        return { handlers };
      });
    };
  },
  getHandler: (panelInstanceId: string): CanvasDropHandler | null =>
    dropHandlerStore.getState().handlers[panelInstanceId]?.handler ?? null,
};
