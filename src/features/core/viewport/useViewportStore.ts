import { create } from 'zustand';
import { GraphPosition } from '@/shared/types/domain';
import { DEFAULT_VIEWPORT } from '@/app/appConfig/default';

interface ViewportStore {
  viewports: Record<string, GraphPosition>;
  setViewport: (groupId: string, updater: Partial<GraphPosition> | ((prev: GraphPosition) => GraphPosition)) => void;
}


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

export function getViewport(groupId: string): GraphPosition {
  return useViewportStore.getState().viewports[groupId] || DEFAULT_VIEWPORT;
}

export function subscribeToViewport(
  groupId: string,
  listener: (viewport: GraphPosition) => void,
): () => void {
  let previous = getViewport(groupId);

  return useViewportStore.subscribe((state) => {
    const next = state.viewports[groupId] || DEFAULT_VIEWPORT;
    if (next === previous) return;

    previous = next;
    listener(next);
  });
}
