import { useGraphDraftStore } from "@/features/core/graphDraft";
import { useActiveEditorGroup } from "./editorGroupContext";

/** Frontend-draft undo/redo availability for the focused Graph editor. */
export function useEditorHistoryAvailability() {
  const { activeResourceRef } = useActiveEditorGroup();
  const session = useGraphDraftStore((state) =>
    activeResourceRef ? state.sessions[activeResourceRef] : undefined,
  );
  const pending = session?.saving === true;

  return {
    canUndo: Boolean(session?.undoStack.length) && !pending,
    canRedo: Boolean(session?.redoStack.length) && !pending,
    pending,
    activeResourceRef,
  };
}
