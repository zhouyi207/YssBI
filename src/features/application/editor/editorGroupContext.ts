import { useLayoutPortSnapshot, workbenchLayoutRead } from "@/modules/workbench/public";

/** Read the native central selection; hydration bookkeeping is not an active-tab fallback. */
export function getActiveGraphContext() {
  const panel = workbenchLayoutRead.getActiveEditorPanel();
  const kind = panel?.metadata.resourceKind;
  if (!panel || (kind !== "event" && kind !== "function")) return null;
  return { groupId: panel.groupId, graphPath: panel.metadata.resourceRef, kind };
}

export function useActiveGraphContext() {
  useLayoutPortSnapshot(workbenchLayoutRead);
  return getActiveGraphContext();
}
