import { useShallow } from "zustand/react/shallow";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import { useActiveGraphContext } from "./editorGroupContext";

/** Rust-owned undo/redo availability for the focused Graph editor. */
export function useEditorHistoryAvailability() {
  const activeResourceRef = useActiveGraphContext()?.graphPath ?? null;
  return useResourceStore(
    useShallow((state) => {
      const session = activeResourceRef ? state.sessions[activeResourceRef] : undefined;
      const pending = session?.saving === true;
      return {
        canUndo: Boolean(session?.canUndo) && !pending,
        canRedo: Boolean(session?.canRedo) && !pending,
        pending,
        activeResourceRef,
      };
    }),
  );
}
