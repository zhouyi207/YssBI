import { useGraphSessionStore } from "@/features/core/graphSession/graphSessionStore";
import { touchGraphDocument } from "./graphDocumentCachePolicy";

/** Focus bookkeeping only; visibility and explicit resource use cases own loading. */
export function focusGraphPanelSession(graphPath: string, groupId: string): void {
  useGraphSessionStore.getState().setFocusedSession(groupId, graphPath);
  touchGraphDocument(graphPath);
}

/** Clear session only when the closed panel owned the focused graph. */
export function deactivateGraphPanelSession(
  groupId: string,
  closedGraphPath?: string | null,
): void {
  const store = useGraphSessionStore.getState();
  const focused = store.focusedSession;
  if (focused?.groupId !== groupId) return;
  if (closedGraphPath != null && focused.graphPath !== closedGraphPath) return;
  store.clearFocusedSession(groupId);
}
