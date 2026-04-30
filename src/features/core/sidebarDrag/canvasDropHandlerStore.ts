/**
 * Store for registering the Canvas drop handler per group.
 * Workspace calls this when a sidebar node template is dropped on the canvas.
 */
import { createStore } from "zustand/vanilla";

type DropHandler = (
  dragState: { type: string; template: any; x: number; y: number },
  event: { altKey: boolean; ctrlKey: boolean }
) => void | Promise<void>;

interface CanvasDropHandlerState {
  handlers: Record<string, DropHandler | undefined>;
  setHandler: (groupId: string, handler: DropHandler | null) => void;
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
  setHandler: (groupId: string, h: DropHandler | null) => {
    dropHandlerStore.getState().setHandler(groupId, h);
  },
  getHandler: (groupId: string): DropHandler | null =>
    dropHandlerStore.getState().handlers[groupId] ?? null,
  subscribe: dropHandlerStore.subscribe,
};
