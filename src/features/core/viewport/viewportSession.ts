import type { GraphPosition } from '@/shared/types/domain';
import { DEFAULT_VIEWPORT } from '@/app/appConfig/default';
import { useViewportStore } from './useViewportStore';

function viewportEqual(a: GraphPosition, b: GraphPosition): boolean {
  return a.x === b.x && a.y === b.y && a.scale === b.scale;
}

function storeViewport(graphPath: string): GraphPosition {
  return useViewportStore.getState().viewports[graphPath] ?? DEFAULT_VIEWPORT;
}

const liveByGraph = new Map<string, GraphPosition>();
const listenersByGraph = new Map<string, Set<(viewport: GraphPosition) => void>>();

function notify(graphPath: string, viewport: GraphPosition): void {
  listenersByGraph.get(graphPath)?.forEach((listener) => listener(viewport));
}

/** Authoritative in-memory viewport (live preview ⊃ committed store). */
export function getViewport(graphPath: string): GraphPosition {
  return liveByGraph.get(graphPath) ?? storeViewport(graphPath);
}

export function setViewportLive(
  graphPath: string,
  updater: Partial<GraphPosition> | ((prev: GraphPosition) => GraphPosition),
): void {
  const prev = getViewport(graphPath);
  const next = typeof updater === 'function' ? updater(prev) : { ...prev, ...updater };
  if (viewportEqual(prev, next)) return;
  liveByGraph.set(graphPath, next);
  notify(graphPath, next);
}

/** Flush live viewport into zustand (persistence / cross-panel reads). */
export function commitViewport(graphPath: string): void {
  const live = liveByGraph.get(graphPath);
  if (!live) return;
  useViewportStore.getState().setViewport(graphPath, live);
}

export function resetLiveViewports(graphPath?: string): void {
  if (graphPath) liveByGraph.delete(graphPath);
  else liveByGraph.clear();
}

export function subscribeToViewport(
  graphPath: string,
  listener: (viewport: GraphPosition) => void,
): () => void {
  const set = listenersByGraph.get(graphPath) ?? new Set();
  set.add(listener);
  listenersByGraph.set(graphPath, set);
  listener(getViewport(graphPath));

  const unsubStore = useViewportStore.subscribe((state, prevState) => {
    const next = state.viewports[graphPath] ?? DEFAULT_VIEWPORT;
    const prev = prevState.viewports[graphPath] ?? DEFAULT_VIEWPORT;
    if (viewportEqual(next, prev)) return;
    liveByGraph.set(graphPath, next);
    listener(next);
  });

  return () => {
    set.delete(listener);
    unsubStore();
  };
}

export function scheduleViewportCommit(
  graphPath: string,
  timers: { commit?: number | null },
  delayMs = 80,
): void {
  if (timers.commit != null) window.clearTimeout(timers.commit);
  timers.commit = window.setTimeout(() => {
    timers.commit = null;
    commitViewport(graphPath);
  }, delayMs);
}

export function scheduleViewportPersist(
  graphPath: string,
  persist: () => void,
  timers: { persist?: number | null },
  delayMs = 300,
): void {
  if (timers.persist != null) window.clearTimeout(timers.persist);
  timers.persist = window.setTimeout(() => {
    timers.persist = null;
    commitViewport(graphPath);
    persist();
  }, delayMs);
}

export function clearViewportTimers(timers: { commit?: number | null; persist?: number | null }): void {
  if (timers.commit != null) window.clearTimeout(timers.commit);
  if (timers.persist != null) window.clearTimeout(timers.persist);
}
