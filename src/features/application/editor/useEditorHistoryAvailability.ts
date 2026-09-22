import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { useActiveGraphContext } from "./editorGroupContext";

/** Rust-owned undo/redo availability for the focused Graph editor. */
export function useEditorHistoryAvailability() {
  const activeResourceRef = useActiveGraphContext()?.graphPath ?? null;
  const session = useGraphProjectionStore((state) =>
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
