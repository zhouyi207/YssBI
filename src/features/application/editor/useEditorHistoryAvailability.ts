import { useGraphEditingStore } from "@/features/core/graphEditing";
import { useActiveEditorGroup } from "./editorGroupContext";

/** Rust-owned undo/redo availability for the focused Graph editor. */
export function useEditorHistoryAvailability() {
  const { activeResourceRef } = useActiveEditorGroup();
  const session = useGraphEditingStore((state) =>
    activeResourceRef ? state.sessions[activeResourceRef] : undefined,
  );
  const pending = session?.saving === true;

  return {
    canUndo: Boolean(session?.canUndo) && !pending,
    canRedo: Boolean(session?.canRedo) && !pending,
    pending,
    activeResourceRef,
  };
}
