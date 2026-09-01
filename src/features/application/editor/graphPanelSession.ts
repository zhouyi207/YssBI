import { useGraphSessionStore } from "@/features/core/graphSession/graphSessionStore";
import { useGraphDataStore } from "@/features/core/dataStore";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import { markResourceLoaded } from "@/features/core/resource";
import { ensureEditorViewport } from "@/features/core/viewport";
import { editorViewportScope } from "@/features/core/viewport/viewportScope";
import { inferGraphResourceKind } from "@/shared/types/domain/graphResourcePath";
import { unloadGraphDocument } from "./graphDocumentUnload";
import { logger } from "@/features/application/observability/appLogger";
import { enforceGraphDocumentCacheLimit, touchGraphDocument } from "./graphDocumentCachePolicy";

let graphCleanupChain: Promise<void> = Promise.resolve();

function scheduleGraphCleanup(previousGraphPath?: string): void {
  graphCleanupChain = graphCleanupChain
    .then(async () => {
      if (previousGraphPath) await unloadGraphDocument(previousGraphPath);
      await enforceGraphDocumentCacheLimit();
    })
    .catch((error) => {
      logger.graph.warn(
        `Background graph cleanup failed: ${error instanceof Error ? error.message : String(error)}`,
        "graphPanelSession",
      );
    });
}

function finishGraphEditorActivation(groupId: string, graphPath: string): void {
  const kind = inferGraphResourceKind(graphPath);
  if (kind) {
    markResourceLoaded({ id: graphPath, kind }, true);
  }
  ensureEditorViewport(editorViewportScope(groupId, graphPath));
}

/** Session bookkeeping + single loadGraph entry + editor activation. */
export async function activateGraphPanelSession(
  graphPath: string,
  groupId: string,
): Promise<boolean> {
  const sessionStore = useGraphSessionStore.getState();
  const previous = sessionStore.setFocusedSession(groupId, graphPath);

  touchGraphDocument(graphPath);

  const loaded = await useProjectIOStore.getState().loadGraph(graphPath);
  if (!loaded || !useGraphDataStore.getState().hasGraph(graphPath)) {
    const focused = useGraphSessionStore.getState().focusedSession;
    if (focused?.groupId === groupId && focused.graphPath === graphPath) {
      if (previous) {
        sessionStore.setFocusedSession(groupId, previous);
      } else {
        sessionStore.clearFocusedSession(groupId);
      }
    }
    return false;
  }

  finishGraphEditorActivation(groupId, graphPath);
  scheduleGraphCleanup(previous && previous !== graphPath ? previous : undefined);
  return true;
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
