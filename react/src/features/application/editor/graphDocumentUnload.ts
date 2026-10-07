import { resetGraphResultQueries } from "@/features/application/results/runtime";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import { invalidateGraphLoadOwnership } from "@/features/application/project/projectIOStore";
import { useExecutionStore } from "@/features/core/execution";
import { clearCanvasInteractionGraph } from "@/features/core/canvas/canvasInteractionCleanup";
import { enqueueGraphTask } from "@/features/application/graphEditing/graphEditCoordinator";
import type { GraphEditVersionDto } from "@/shared/types/domain/editorMutation";
import { releaseGraphViewport } from "@/features/core/viewport";
import { GraphService } from "@/services/graph/graphService";
import { logger } from "@/utils/frontendLogger";
import { formatApplicationIpcError } from "@/features/application/errorReference";
import { shouldRetainGraphDocument } from "./graphDocumentRetention";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import {
  beginGraphUnloadLifecycle,
  finishGraphUnloadLifecycle,
  isGraphLifecycleCurrent,
} from "@/features/application/graphProjection/graphProjectionLifecycle";

/** Unload frontend/backend graph cache when retention guards no longer apply. */
export async function unloadGraphDocument(
  graphPath: string,
  discardVersion?: GraphEditVersionDto,
): Promise<void> {
  if (shouldRetainGraphDocument(graphPath, !discardVersion)) return;

  let identity: ProjectIdentitySnapshot;
  try {
    identity = captureProjectIdentity();
  } catch {
    return;
  }
  const lifecycleToken = beginGraphUnloadLifecycle(graphPath);
  const isCurrent = () =>
    isCurrentProjectIdentity(identity) && isGraphLifecycleCurrent(graphPath, lifecycleToken);
  invalidateGraphLoadOwnership(graphPath);

  try {
    await enqueueGraphTask(
      graphPath,
      async () => {
        if (!isCurrent()) return;
        if (shouldRetainGraphDocument(graphPath, !discardVersion)) return;
        const removed = await GraphService.unloadProjectGraph(
          graphPath,
          lifecycleToken,
          identity.projectInstanceId,
          discardVersion,
        );
        if (!isCurrent()) return;
        if (!removed) return;
        useResourceStore.getState().removeGraphSession(graphPath);
        if (!isCurrent()) return;
        resetGraphResultQueries(graphPath, isCurrent);
        if (!isCurrent()) return;
        clearCanvasInteractionGraph(graphPath, isCurrent);
        if (!isCurrent()) return;
        useExecutionStore.getState().releaseGraphExecutionState(graphPath);
        if (!isCurrent()) return;
        releaseGraphViewport(graphPath);
      },
      undefined,
    );
  } catch (error) {
    if (!isCurrent()) return;
    logger.graph.warn(
      `Failed to unload graph '${graphPath}': ${formatApplicationIpcError(error)}`,
      "unloadGraphDocument",
    );
  } finally {
    finishGraphUnloadLifecycle(graphPath, lifecycleToken);
  }
}
