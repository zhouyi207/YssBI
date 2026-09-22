import { resetGraphResultQueries } from "@/features/application/results/runtime";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { invalidateGraphLoadOwnership } from "@/features/application/project/projectIOStore";
import { useExecutionStore } from "@/features/core/execution";
import { clearCanvasInteractionGraph } from "@/features/core/canvas/canvasInteractionCleanup";
import { markResourceLoaded, clearResourceDocumentState } from "@/features/core/resource";
import { enqueueGraphTask } from "@/features/application/graphEditing/graphEditCoordinator";
import type { GraphEditVersionDto } from "@/shared/types/domain/editorMutation";
import { releaseGraphViewport } from "@/features/core/viewport";
import { getGraphResourceKind } from "@/features/core/resource/resourceSelectors";
import { GraphService } from "@/services/graph/graphService";
import { logger } from "@/features/application/observability/appLogger";
import { shouldRetainGraphDocument } from "./graphDocumentRetention";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import {
  beginGraphUnloadLifecycle,
  isGraphLifecycleCurrent,
} from "@/features/application/graphProjection/graphProjectionLifecycle";

/** Unload frontend/backend graph cache when retention guards no longer apply. */
export async function unloadGraphDocument(
  graphPath: string,
  discardVersion?: GraphEditVersionDto,
): Promise<void> {
  if (shouldRetainGraphDocument(graphPath, !discardVersion)) return;

  const lifecycleToken = beginGraphUnloadLifecycle(graphPath);
  invalidateGraphLoadOwnership(graphPath);

  const kind = getGraphResourceKind(graphPath);
  if (kind) {
    markResourceLoaded({ id: graphPath, kind }, false);
  }

  let identity: ProjectIdentitySnapshot;
  try {
    identity = captureProjectIdentity();
  } catch {
    return;
  }

  try {
    await enqueueGraphTask(
      graphPath,
      async () => {
        if (
          !isCurrentProjectIdentity(identity) ||
          !isGraphLifecycleCurrent(graphPath, lifecycleToken)
        )
          return;
        if (shouldRetainGraphDocument(graphPath, !discardVersion)) {
          if (kind) markResourceLoaded({ id: graphPath, kind }, true);
          return;
        }
        const removed = await GraphService.unloadProjectGraph(
          graphPath,
          lifecycleToken,
          identity.projectInstanceId,
          discardVersion,
        );
        if (
          !isCurrentProjectIdentity(identity) ||
          !isGraphLifecycleCurrent(graphPath, lifecycleToken)
        )
          return;
        if (!removed) {
          if (kind) markResourceLoaded({ id: graphPath, kind }, true);
          return;
        }
        useGraphProjectionStore.getState().clearGraph(graphPath);
        resetGraphResultQueries(graphPath);
        clearCanvasInteractionGraph(graphPath);
        useExecutionStore.getState().releaseGraphExecutionState(graphPath);
        releaseGraphViewport(graphPath);
        if (kind) clearResourceDocumentState({ id: graphPath, kind });
      },
      undefined,
    );
  } catch (error) {
    if (!isCurrentProjectIdentity(identity)) return;
    if (kind && isGraphLifecycleCurrent(graphPath, lifecycleToken))
      markResourceLoaded({ id: graphPath, kind }, true);
    logger.graph.warn(
      `Failed to unload graph '${graphPath}': ${error instanceof Error ? error.message : String(error)}`,
      "unloadGraphDocument",
    );
  }
}
