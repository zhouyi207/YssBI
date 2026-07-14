/**
 * Store for registering the Canvas drop handler per group.
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
  setHandler: (groupId: string, handler: CanvasDropHandler | null) => void;
}

const dropHandlerStore = createStore<CanvasDropHandlerState>((set) => ({
  handlers: {},
  setHandler: (groupId, handler) =>
    set((state) => {
      const handlers = { ...state.handlers };
      if (handler) {
        handlers[groupId] = handler;
      } else {
        delete handlers[groupId];
      }
      return { handlers };
    }),
}));

export const canvasDropHandlerStore = {
  setHandler: (groupId: string, h: CanvasDropHandler | null) => {
    dropHandlerStore.getState().setHandler(groupId, h);
  },
  getHandler: (groupId: string): CanvasDropHandler | null =>
    dropHandlerStore.getState().handlers[groupId] ?? null,
  subscribe: dropHandlerStore.subscribe,
};
