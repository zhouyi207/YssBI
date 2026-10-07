import { workbenchLayoutRead } from "@/modules/workbench/public";
import { useGraphSessionStore } from "@/features/core/graphSession/graphSessionStore";
import { isGraphResourceDirty } from "@/features/core/resource";

/** Keep hydrated graph data while it is focused, open in a tab, or dirty. */
export function shouldRetainGraphDocument(graphPath: string, retainDirty = true): boolean {
  if (useGraphSessionStore.getState().isFocusedGraphPath(graphPath)) return true;
  if (workbenchLayoutRead.findEditorPanelsByResource(graphPath).length > 0) return true;
  if (retainDirty && isGraphResourceDirty(graphPath)) return true;
  return false;
}
