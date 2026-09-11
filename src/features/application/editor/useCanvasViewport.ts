import { useCallback, useMemo, useSyncExternalStore } from "react";
import {
  editorViewportScope,
  getViewport,
  subscribeToViewport,
  setViewportLive,
  commitViewport,
  persistGraphViewport,
  type EditorViewport,
} from "@/features/core/viewport";

/** One viewport owner shared by the renderer, sidebar drops, and navigation commands. */
export function useCanvasViewport(groupId: string, graphPath: string) {
  const scope = useMemo(() => editorViewportScope(groupId, graphPath), [groupId, graphPath]);
  const subscribe = useCallback(
    (listener: () => void) => subscribeToViewport(scope, listener),
    [scope],
  );
  const read = useCallback(() => getViewport(scope), [scope]);
  const viewport = useSyncExternalStore(subscribe, read, read);
  const setViewport = useCallback((next: EditorViewport) => setViewportLive(scope, next), [scope]);
  const commit = useCallback(() => {
    commitViewport(scope);
    persistGraphViewport(scope);
  }, [scope]);
  return { viewport, setViewport, commit };
}
