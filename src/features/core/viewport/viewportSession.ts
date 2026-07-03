import type { GraphPosition } from '@/shared/types/domain';
import { DEFAULT_VIEWPORT } from '@/app/appConfig/default';
import { useViewportStore } from './useViewportStore';

function viewportEqual(a: GraphPosition, b: GraphPosition): boolean {
  return a.x === b.x && a.y === b.y && a.scale === b.scale;
}

function storeViewport(graphId: string): GraphPosition {
  return useViewportStore.getState().viewports[graphId] ?? DEFAULT_VIEWPORT;
}

const liveByGraph = new Map<string, GraphPosition>();
const listenersByGraph = new Map<string, Set<(viewport: GraphPosition) => void>>();

function notify(graphId: string, viewport: GraphPosition): void {
  listenersByGraph.get(graphId)?.forEach((listener) => listener(viewport));
}

/** Authoritative in-memory viewport (live preview ⊃ committed store). */
export function getViewport(graphId: string): GraphPosition {
  return liveByGraph.get(graphId) ?? storeViewport(graphId);
}

export function setViewportLive(
  graphId: string,
  updater: Partial<GraphPosition> | ((prev: GraphPosition) => GraphPosition),
): void {
  const prev = getViewport(graphId);
  const next = typeof updater === 'function' ? updater(prev) : { ...prev, ...updater };
  if (viewportEqual(prev, next)) return;
  liveByGraph.set(graphId, next);
  notify(graphId, next);
}

/** Flush live viewport into zustand (persistence / cross-panel reads). */
export function commitViewport(graphId: string): void {
  const live = liveByGraph.get(graphId);
  if (!live) return;
  useViewportStore.getState().setViewport(graphId, live);
}

export function resetLiveViewports(graphId?: string): void {
  if (graphId) liveByGraph.delete(graphId);
  else liveByGraph.clear();
}

export function subscribeToViewport(
  graphId: string,
  listener: (viewport: GraphPosition) => void,
): () => void {
  const set = listenersByGraph.get(graphId) ?? new Set();
  set.add(listener);
  listenersByGraph.set(graphId, set);
  listener(getViewport(graphId));

  const unsubStore = useViewportStore.subscribe((state, prevState) => {
    const next = state.viewports[graphId] ?? DEFAULT_VIEWPORT;
    const prev = prevState.viewports[graphId] ?? DEFAULT_VIEWPORT;
    if (viewportEqual(next, prev)) return;
    liveByGraph.set(graphId, next);
    listener(next);
  });

  return () => {
    set.delete(listener);
    unsubStore();
  };
}

export function scheduleViewportCommit(
  graphId: string,
  timers: { commit?: number | null },
  delayMs = 80,
): void {
  if (timers.commit != null) window.clearTimeout(timers.commit);
  timers.commit = window.setTimeout(() => {
    timers.commit = null;
    commitViewport(graphId);
  }, delayMs);
}

export function scheduleViewportPersist(
  graphId: string,
  persist: () => void,
  timers: { persist?: number | null },
  delayMs = 300,
): void {
  if (timers.persist != null) window.clearTimeout(timers.persist);
  timers.persist = window.setTimeout(() => {
    timers.persist = null;
    commitViewport(graphId);
    persist();
  }, delayMs);
}

export function clearViewportTimers(timers: { commit?: number | null; persist?: number | null }): void {
  if (timers.commit != null) window.clearTimeout(timers.commit);
  if (timers.persist != null) window.clearTimeout(timers.persist);
}
