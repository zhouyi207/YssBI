import { create } from 'zustand';
import { GraphPosition } from '@/shared/types/domain';
import { DEFAULT_VIEWPORT } from '@/app/appConfig/default';
import { resetLiveViewports } from './viewportSession';

interface ViewportStore {
  /** Committed viewports (persisted / loaded from project). Live gesture preview stays in viewportSession. */
  viewports: Record<string, GraphPosition>;
  setViewport: (
    graphPath: string,
    updater: Partial<GraphPosition> | ((prev: GraphPosition) => GraphPosition),
  ) => void;
  clear: () => void;
}

export const useViewportStore = create<ViewportStore>((set) => ({
  viewports: {},
  setViewport: (graphPath, updater) =>
    set((state) => {
      const current = state.viewports[graphPath] ?? { ...DEFAULT_VIEWPORT };
      const next = typeof updater === 'function' ? updater(current) : { ...current, ...updater };

      if (current.x === next.x && current.y === next.y && current.scale === next.scale) {
        return state;
      }

      return {
        viewports: {
          ...state.viewports,
          [graphPath]: next,
        },
      };
    }),
  clear: () => {
    resetLiveViewports();
    set({ viewports: {} });
  },
}));

export function remapGraphViewport(from: string, to: string): void {
  if (from === to) return;
  resetLiveViewports(from);
  useViewportStore.setState((state) => {
    const viewport = state.viewports[from];
    if (!viewport) return state;
    const viewports = { ...state.viewports };
    delete viewports[from];
    viewports[to] = viewport;
    return { viewports };
  });
}

export function normalizeGraphCanvas(canvas?: GraphPosition | null): GraphPosition {
  if (!canvas) return { ...DEFAULT_VIEWPORT };
  return {
    x: canvas.x ?? 0,
    y: canvas.y ?? 0,
    scale: canvas.scale ?? DEFAULT_VIEWPORT.scale,
  };
}

export function applyGraphViewport(graphPath: string, canvas?: GraphPosition | null): void {
  useViewportStore.getState().setViewport(graphPath, normalizeGraphCanvas(canvas));
}

/** 首次打开 tab 时从 graph.canvas 恢复；会话内已 pan 过则保留内存值 */
export function ensureGraphViewport(graphPath: string, canvas?: GraphPosition | null): void {
  if (useViewportStore.getState().viewports[graphPath]) return;
  applyGraphViewport(graphPath, canvas);
}

/** Drop committed + live viewport when a graph document is unloaded from memory. */
export function releaseGraphViewport(graphPath: string): void {
  resetLiveViewports(graphPath);
  useViewportStore.setState((state) => {
    if (!(graphPath in state.viewports)) return state;
    const viewports = { ...state.viewports };
    delete viewports[graphPath];
    return { viewports };
  });
}

export function syncGraphViewportsFromRecords(
  graphs: Record<string, { canvas?: GraphPosition | null }>,
): void {
  for (const [graphPath, graph] of Object.entries(graphs)) {
    applyGraphViewport(graphPath, graph.canvas);
  }
}
