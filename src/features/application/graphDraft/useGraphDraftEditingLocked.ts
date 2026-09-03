import { useGraphDraftStore } from "@/features/core/graphDraft";

/** Projects the Graph Draft save lock into editing surfaces. */
export function useGraphDraftEditingLocked(graphPath: string): boolean {
  return useGraphDraftStore((state) => state.sessions[graphPath]?.saving === true);
}
