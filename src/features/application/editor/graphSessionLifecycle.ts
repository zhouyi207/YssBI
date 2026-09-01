import { useGraphSessionStore } from "@/features/core/graphSession/graphSessionStore";
import { enforceGraphDocumentCacheLimit } from "./graphDocumentCachePolicy";

/** Clear focused session for a suspended group; open tabs retain graph data via retention guards. */
export async function suspendEditorGroupGraphSession(groupId: string): Promise<void> {
  useGraphSessionStore.getState().clearFocusedSession(groupId);
  await enforceGraphDocumentCacheLimit();
}
