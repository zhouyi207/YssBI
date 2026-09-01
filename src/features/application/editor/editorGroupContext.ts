import { useActiveEditorGroup as useCoreActiveEditorGroup } from "@/features/core/editor";

export function useActiveEditorGroup(overrideGroupId?: string | null) {
  return useCoreActiveEditorGroup(overrideGroupId);
}
