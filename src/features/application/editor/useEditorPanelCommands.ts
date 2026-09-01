import { useCallback } from "react";

import { openGraphInEditor } from "./openGraphInEditor";
import { splitEditorPanel } from "./editorGroupCommands";

/** Tab Management Hook — thin React facade over canonical editor commands. */
export function useEditorPanelCommands() {
  const openGraph = useCallback(
    async (
      id: string,
      name: string,
      type: "event" | "function",
      options?: { pinned?: boolean; targetGroupId?: string },
    ): Promise<void> => {
      await openGraphInEditor(id, name, type, options?.targetGroupId, { pinned: options?.pinned });
    },
    [],
  );

  const splitEditorRight = useCallback((sourceGroupId: string) => {
    void splitEditorPanel(sourceGroupId, "right");
  }, []);

  return { openGraph, splitEditorRight };
}
