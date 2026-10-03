import { readLiveViewport, subscribeLiveViewport, writeLiveViewport } from "./liveViewportState";
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

/** Authoritative in-memory viewport for one editor pane (live preview ⊃ committed store). */
export function getViewport(scope: ViewportScope): EditorViewport {
  return readLiveViewport(scope) ?? storeViewport(scope);
}

export function setViewportLive(
  scope: ViewportScope,
  updater: Partial<EditorViewport> | ((prev: EditorViewport) => EditorViewport),
): void {
  const prev = getViewport(scope);
  const next = typeof updater === "function" ? updater(prev) : { ...prev, ...updater };
  if (viewportEqual(prev, next)) return;
  writeLiveViewport(scope, next);
}

/** Flush live viewport into zustand (persistence / cross-panel reads). */
export function commitViewport(scope: ViewportScope): void {
  const live = readLiveViewport(scope);
  if (!live) return;
  useViewportStore.getState().setViewport(scope, live);
}

export function subscribeToViewport(
  scope: ViewportScope,
  listener: (viewport: EditorViewport) => void,
): () => void {
  const unsubscribe = subscribeLiveViewport(scope, (viewport) => {
    listener(viewport ?? storeViewport(scope));
  });
  listener(getViewport(scope));
  return unsubscribe;
}
