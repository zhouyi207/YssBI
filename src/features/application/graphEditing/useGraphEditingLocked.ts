import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";

/** Projects the Graph Draft save lock into editing surfaces. */
export function useGraphEditingLocked(graphPath: string): boolean {
  return useGraphProjectionStore((state) => state.sessions[graphPath]?.saving === true);
}
