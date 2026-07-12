import { useGraphDataStore, useVariableStore } from '@/features/core/dataStore';
import { useExecutionStore } from '@/features/core/execution';
import { markResourceLoaded } from '@/features/core/resource';
import { releaseGraphViewport } from '@/features/core/viewport';
import { inferGraphResourceKind } from '@/shared/types/domain/graphResourcePath';
import { GraphService } from '@/services/graph/graphService';
import { logger } from '@/utils/appLogger';
import { shouldRetainGraphDocument } from './graphDocumentRetention';

/** Unload frontend/backend graph cache when retention guards no longer apply. */
export async function unloadGraphDocument(graphPath: string): Promise<void> {
  if (shouldRetainGraphDocument(graphPath)) return;

  useGraphDataStore.getState().clearGraph(graphPath);
  useVariableStore.getState().clearGraphVariables(graphPath);
  useExecutionStore.getState().releaseGraphExecutionState(graphPath);
  releaseGraphViewport(graphPath);

  const kind = inferGraphResourceKind(graphPath);
  if (kind) {
    markResourceLoaded({ id: graphPath, kind }, false);
  }

  try {
    await GraphService.unloadProjectGraph(graphPath);
  } catch (error) {
    logger.graph.warn(
      `Failed to unload graph '${graphPath}': ${error instanceof Error ? error.message : String(error)}`,
      'unloadGraphDocument',
    );
  }
}
