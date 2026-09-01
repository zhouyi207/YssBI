import { workbenchDockviewRead } from "@/modules/workbench/public";
import { useGraphSessionStore } from "@/features/core/graphSession/graphSessionStore";
import { isGraphResourceDirty } from "@/features/core/resource";

/** Keep hydrated graph data while it is focused, open in a tab, or dirty. */
export function shouldRetainGraphDocument(graphPath: string): boolean {
  if (useGraphSessionStore.getState().isFocusedGraphPath(graphPath)) return true;
  if (workbenchDockviewRead.findEditorPanelsByResource(graphPath).length > 0) return true;
  if (isGraphResourceDirty(graphPath)) return true;
  return false;
}
