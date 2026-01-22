import { create } from 'zustand';
import { CanvasState } from '../Types/canvas';

interface ViewportStore {
  viewports: Record<string, CanvasState>;
  setViewport: (groupId: string, updater: Partial<CanvasState> | ((prev: CanvasState) => CanvasState)) => void;
}

const DEFAULT_VIEWPORT: CanvasState = { x: 0, y: 0, scale: 1 };

export const useViewportStore = create<ViewportStore>((set) => ({
  viewports: {
    'main-group': { ...DEFAULT_VIEWPORT },
  },
  setViewport: (groupId, updater) => set((state) => {
    const current = state.viewports[groupId] || { ...DEFAULT_VIEWPORT };
    const next = typeof updater === 'function' ? updater(current) : { ...current, ...updater };
    
    if (current.x === next.x && current.y === next.y && current.scale === next.scale) {
      return state;
    }
    
    return {
      viewports: {
        ...state.viewports,
        [groupId]: next,
      },
    };
  }),
}));
