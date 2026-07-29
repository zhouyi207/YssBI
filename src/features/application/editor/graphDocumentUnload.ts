import {
  invalidateGraphLoadOwnership,
  useGraphDataStore,

  useVariableStore,
} from '@/features/core/dataStore';
import { useExecutionStore } from '@/features/core/execution';
import { useGraphInteractionStore } from '@/features/core/graphInteraction';
import { markResourceLoaded } from '@/features/core/resource';
import { releaseGraphViewport } from '@/features/core/viewport';
import { inferGraphResourceKind } from '@/shared/types/domain/graphResourcePath';
import { GraphService } from '@/services/graph/graphService';
import { logger } from '@/utils/appLogger';
import { shouldRetainGraphDocument } from './graphDocumentRetention';
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from '@/services/project/projectIdentity';
import {
  beginGraphUnloadLifecycle,
  isGraphLifecycleCurrent,
} from '@/features/application/editorProjection/graphProjectionCoordinator';

/** Unload frontend/backend graph cache when retention guards no longer apply. */
export async function unloadGraphDocument(graphPath: string): Promise<void> {
  if (shouldRetainGraphDocument(graphPath)) return;

  const lifecycleToken = beginGraphUnloadLifecycle(graphPath);
  invalidateGraphLoadOwnership(graphPath);
  useGraphDataStore.getState().clearGraph(graphPath);
  useGraphInteractionStore.getState().clearGraphInteraction(graphPath);
  useVariableStore.getState().clearGraphVariables(graphPath);
  useExecutionStore.getState().releaseGraphExecutionState(graphPath);
  releaseGraphViewport(graphPath);

  const kind = inferGraphResourceKind(graphPath);
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
    await GraphService.unloadProjectGraph(
      graphPath,
      lifecycleToken,
      identity.projectInstanceId,
    );
    if (!isCurrentProjectIdentity(identity)) return;
    if (kind && isGraphLifecycleCurrent(graphPath, lifecycleToken)) {
      markResourceLoaded({ id: graphPath, kind }, false);
    }
  } catch (error) {
    if (!isCurrentProjectIdentity(identity)) return;
    logger.graph.warn(
      `Failed to unload graph '${graphPath}': ${error instanceof Error ? error.message : String(error)}`,
      'unloadGraphDocument',
    );
  }
}
