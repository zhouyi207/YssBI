import type { EditorViewport } from "./editorViewport";
import { viewportScopeKey, type ViewportScope } from "./viewportScope";

const liveViewports = new Map<string, EditorViewport>();

export function readLiveViewport(scope: ViewportScope): EditorViewport | undefined {
  return liveViewports.get(viewportScopeKey(scope));
}

export function writeLiveViewport(scope: ViewportScope, viewport: EditorViewport): void {
  liveViewports.set(viewportScopeKey(scope), viewport);
}

export function resetLiveViewports(scope?: ViewportScope): void {
  if (scope) liveViewports.delete(viewportScopeKey(scope));
  else liveViewports.clear();
}
