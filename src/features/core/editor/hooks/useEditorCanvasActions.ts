import { useCallback, type RefObject } from "react";
import { commitViewport, setViewportLive, editorViewportScope } from "@/features/core/viewport";
import type { EditorViewport } from "@/features/core/viewport";

export function useEditorCanvasActions(
  activeGroupIdRef: RefObject<string>,
  activeResourceRefRef: RefObject<string | null>,
) {
  const setCanvas = useCallback(
    (
      updater: EditorViewport | ((prev: EditorViewport) => EditorViewport),
      targetGraphPath?: string,
    ) => {
      const graphPath = targetGraphPath ?? activeResourceRefRef.current;
      const groupId = activeGroupIdRef.current;
      if (!graphPath || !groupId) return;
      const scope = editorViewportScope(groupId, graphPath);
      setViewportLive(scope, updater);
      commitViewport(scope);
    },
    [activeGroupIdRef, activeResourceRefRef],
  );

  return { setCanvas };
}
