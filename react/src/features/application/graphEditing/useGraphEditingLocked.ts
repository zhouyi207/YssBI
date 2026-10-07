import { useResourceStore } from "@/features/core/resource/resourceStore";

/** Projects the Graph Draft save lock into editing surfaces. */
export function useGraphEditingLocked(graphPath: string): boolean {
  return useResourceStore((state) => state.sessions[graphPath]?.saving === true);
}
