import type { EditorViewport } from "./editorViewport";
import { DEFAULT_VIEWPORT } from "@/shared/config-default";
import { useViewportStore } from "./useViewportStore";
import type { ViewportScope } from "./viewportScope";
import { viewportScopeKey } from "./viewportScope";

function viewportEqual(a: EditorViewport, b: EditorViewport): boolean {
  return a.x === b.x && a.y === b.y && a.scale === b.scale;
}

function storeViewport(scope: ViewportScope): EditorViewport {
  return useViewportStore.getState().viewports[viewportScopeKey(scope)] ?? DEFAULT_VIEWPORT;
}

const liveByScope = new Map<string, EditorViewport>();
const listenersByScope = new Map<string, Set<(viewport: EditorViewport) => void>>();

function notify(scope: ViewportScope, viewport: EditorViewport): void {
  listenersByScope.get(viewportScopeKey(scope))?.forEach((listener) => listener(viewport));
}

/** Authoritative in-memory viewport for one editor pane (live preview ⊃ committed store). */
export function getViewport(scope: ViewportScope): EditorViewport {
  const key = viewportScopeKey(scope);
  return liveByScope.get(key) ?? storeViewport(scope);
}

export function setViewportLive(
  scope: ViewportScope,
  updater: Partial<EditorViewport> | ((prev: EditorViewport) => EditorViewport),
): void {
  const prev = getViewport(scope);
  const next = typeof updater === "function" ? updater(prev) : { ...prev, ...updater };
  if (viewportEqual(prev, next)) return;
  liveByScope.set(viewportScopeKey(scope), next);
  notify(scope, next);
}

/** Flush live viewport into zustand (persistence / cross-panel reads). */
export function commitViewport(scope: ViewportScope): void {
  const key = viewportScopeKey(scope);
  const live = liveByScope.get(key);
  if (!live) return;
  useViewportStore.getState().setViewport(scope, live);
}

export function resetLiveViewports(scope?: ViewportScope): void {
  if (scope) liveByScope.delete(viewportScopeKey(scope));
  else liveByScope.clear();
}

export function subscribeToViewport(
  scope: ViewportScope,
  listener: (viewport: EditorViewport) => void,
): () => void {
  const key = viewportScopeKey(scope);
  const set = listenersByScope.get(key) ?? new Set();
  set.add(listener);
  listenersByScope.set(key, set);
  listener(getViewport(scope));

  const unsubStore = useViewportStore.subscribe((state, prevState) => {
    const next = state.viewports[key] ?? DEFAULT_VIEWPORT;
    const prev = prevState.viewports[key] ?? DEFAULT_VIEWPORT;
    if (viewportEqual(next, prev)) return;
    liveByScope.set(key, next);
    listener(next);
  });

  return () => {
    set.delete(listener);
    unsubStore();
  };
}

export function scheduleViewportCommit(
  scope: ViewportScope,
  timers: { commit?: number | null },
  delayMs = 80,
): void {
  if (timers.commit != null) window.clearTimeout(timers.commit);
  timers.commit = window.setTimeout(() => {
    timers.commit = null;
    commitViewport(scope);
  }, delayMs);
}

export function scheduleViewportPersist(
  scope: ViewportScope,
  persist: () => void,
  timers: { persist?: number | null },
  delayMs = 300,
): void {
  if (timers.persist != null) window.clearTimeout(timers.persist);
  timers.persist = window.setTimeout(() => {
    timers.persist = null;
    commitViewport(scope);
    persist();
  }, delayMs);
}

export function clearViewportTimers(timers: {
  commit?: number | null;
  persist?: number | null;
}): void {
  if (timers.commit != null) window.clearTimeout(timers.commit);
  if (timers.persist != null) window.clearTimeout(timers.persist);
}
