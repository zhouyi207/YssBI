import { useMemo } from "react";
import {
  editorViewportScope,
  getViewport,
  subscribeToViewport,
  setViewportLive,
  commitViewport,
  persistGraphViewport,
  type EditorViewport,
} from "@/features/core/viewport";

export interface EditorCanvasViewportSession {
  getViewport(): EditorViewport;
  subscribe(listener: (viewport: EditorViewport) => void): () => void;
  setViewport(viewport: EditorViewport): void;
  commit(): void;
}

/** Stable access to the shared session; pointer frames do not render the canvas owner. */
export function useCanvasViewport(groupId: string, graphPath: string): EditorCanvasViewportSession {
  return useMemo(() => {
    const scope = editorViewportScope(groupId, graphPath);
    return {
      getViewport: () => getViewport(scope),
      subscribe: (listener) => subscribeToViewport(scope, listener),
      setViewport: (next) => setViewportLive(scope, next),
      commit: () => {
        commitViewport(scope);
        persistGraphViewport(scope);
      },
    };
  }, [groupId, graphPath]);
}
