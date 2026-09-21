import { focusEditorGroupSync, hydrateEditorGroup } from "./activateEditorPanelAndSyncSession";

/** Prepare graph hydration for interaction; native layout selection is managed separately. */
export function prepareEditorGroupForInteraction(groupId: string): void {
  if (focusEditorGroupSync(groupId)) void hydrateEditorGroup(groupId);
}
