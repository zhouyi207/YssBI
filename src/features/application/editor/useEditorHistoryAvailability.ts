import { useEffect } from "react";
import { ensureHistoryStatus } from "@/features/application/editorMutation/historyCoordinator";
import { useHistoryStore } from "@/features/core/history";
import { useActiveEditorGroup } from "@/features/core/editor/hooks/useActiveEditorGroup";

/** Backend-derived undo/redo availability for the focused editor group's active tab. */
export function useEditorHistoryAvailability() {
  const { activeTabId } = useActiveEditorGroup();
  const canUndoFromBackend = useHistoryStore((state) => state.canUndo);
  const canRedoFromBackend = useHistoryStore((state) => state.canRedo);
  const pending = useHistoryStore((state) => state.pending);
  const available = Boolean(activeTabId) && !pending;

  useEffect(() => {
    void ensureHistoryStatus().catch(() => undefined);
  }, []);

  return {
    canUndo: available && canUndoFromBackend,
    canRedo: available && canRedoFromBackend,
    pending,
    activeTabId,
  };
}
