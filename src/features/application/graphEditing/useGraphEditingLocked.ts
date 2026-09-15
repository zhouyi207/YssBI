import { useGraphEditingStore } from "@/features/core/graphEditing";

/** Projects the Graph Draft save lock into editing surfaces. */
export function useGraphEditingLocked(graphPath: string): boolean {
  return useGraphEditingStore((state) => state.sessions[graphPath]?.saving === true);
}
