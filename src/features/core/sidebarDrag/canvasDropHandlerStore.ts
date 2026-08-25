/**
 * Store for registering the Canvas drop handler per Dockview panel.
 * Workspace calls this when a sidebar item is dropped on the canvas.
 */
import { createStore } from "zustand/vanilla";
import type { SidebarDragState } from "@/features/core/dnd";

export type CanvasDropHandler = (
  dragState: SidebarDragState,
  event: { altKey: boolean; ctrlKey: boolean; shiftKey: boolean },
) => void | Promise<boolean>;

interface CanvasDropHandlerState {
  handlers: Record<string, CanvasDropHandler | undefined>;
  setHandler: (panelInstanceId: string, handler: CanvasDropHandler | null) => void;
}

const dropHandlerStore = createStore<CanvasDropHandlerState>((set) => ({
  handlers: {},
  setHandler: (panelInstanceId, handler) =>
    set((state) => {
      const handlers = { ...state.handlers };
      if (handler) {
        handlers[panelInstanceId] = handler;
      } else {
        delete handlers[panelInstanceId];
      }
      return { handlers };
    }),
}));

export const canvasDropHandlerStore = {
  setHandler: (panelInstanceId: string, h: CanvasDropHandler | null) => {
    dropHandlerStore.getState().setHandler(panelInstanceId, h);
  },
  getHandler: (panelInstanceId: string): CanvasDropHandler | null =>
    dropHandlerStore.getState().handlers[panelInstanceId] ?? null,
  subscribe: dropHandlerStore.subscribe,
};
