import { create } from 'zustand';
import { GraphPosition } from '@/shared/types/domain';
import { DEFAULT_VIEWPORT } from '@/app/appConfig/default';

interface ViewportStore {
  /** 按 graphId 存储画布 pan/zoom，与后端 `graph.position` / 文件 `canvas` 对齐 */
  viewports: Record<string, GraphPosition>;
  setViewport: (
    graphId: string,
    updater: Partial<GraphPosition> | ((prev: GraphPosition) => GraphPosition),
  ) => void;
  /** 切换项目时清空 */
  clear: () => void;
}

export const useViewportStore = create<ViewportStore>((set) => ({
  viewports: {},
  setViewport: (graphId, updater) =>
    set((state) => {
      const current = state.viewports[graphId] ?? { ...DEFAULT_VIEWPORT };
      const next = typeof updater === 'function' ? updater(current) : { ...current, ...updater };

      if (current.x === next.x && current.y === next.y && current.scale === next.scale) {
        return state;
      }

      return {
        viewports: {
          ...state.viewports,
          [graphId]: next,
        },
      };
    }),
  clear: () => set({ viewports: {} }),
}));

export function normalizeGraphCanvas(canvas?: GraphPosition | null): GraphPosition {
  if (!canvas) return { ...DEFAULT_VIEWPORT };
  return {
    x: canvas.x ?? 0,
    y: canvas.y ?? 0,
    scale: canvas.scale ?? DEFAULT_VIEWPORT.scale,
  };
}

/** 用 graph 的 canvas 覆盖写入视口（项目加载、懒加载 graph 后） */
export function applyGraphViewport(graphId: string, canvas?: GraphPosition | null): void {
  useViewportStore.getState().setViewport(graphId, normalizeGraphCanvas(canvas));
}

/** 首次打开 tab 时从 graph.canvas 恢复；会话内已 pan 过则保留内存值 */
export function ensureGraphViewport(graphId: string, canvas?: GraphPosition | null): void {
  if (useViewportStore.getState().viewports[graphId]) return;
  applyGraphViewport(graphId, canvas);
}

export function syncGraphViewportsFromRecords(
  graphs: Record<string, { canvas?: GraphPosition | null }>,
): void {
  for (const [graphId, graph] of Object.entries(graphs)) {
    applyGraphViewport(graphId, graph.canvas);
  }
}

export function getViewport(graphId: string): GraphPosition {
  return useViewportStore.getState().viewports[graphId] ?? DEFAULT_VIEWPORT;
}

export function subscribeToViewport(
  graphId: string,
  listener: (viewport: GraphPosition) => void,
): () => void {
  let previous = getViewport(graphId);

  return useViewportStore.subscribe((state) => {
    const next = state.viewports[graphId] ?? DEFAULT_VIEWPORT;
    if (next === previous) return;

    previous = next;
    listener(next);
  });
}
