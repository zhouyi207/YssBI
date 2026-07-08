import { useGraphDataStore, useVariableStore } from '@/features/core/dataStore';
import { isGraphOpenInAnyTab, isGraphTabDirty } from '@/features/core/layout/graphTabQueries';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { markResourceLoaded } from '@/features/core/resource';
import { inferGraphResourceKind } from '@/shared/types/domain/graphResourcePath';
import { GraphService } from '@/services/graph/graphService';
import { logger } from '@/utils/appLogger';

/** Unload backend + frontend cache when a path is no longer active and has no open tab reference. */
export async function deactivateInactiveGraphPath(graphPath: string): Promise<void> {
  if (useGraphSessionStore.getState().isPathActiveInAnyGroup(graphPath)) return;
  if (isGraphOpenInAnyTab(graphPath)) return;
  if (isGraphTabDirty(graphPath)) return;

  useGraphDataStore.getState().clearGraph(graphPath);
  useVariableStore.getState().clearGraphVariables(graphPath);

  const kind = inferGraphResourceKind(graphPath);
  if (kind) {
    markResourceLoaded({ id: graphPath, kind }, false);
  }

  try {
    await GraphService.unloadProjectGraph(graphPath);
  } catch (error) {
    logger.graph.warn(
      `Failed to unload graph '${graphPath}': ${error instanceof Error ? error.message : String(error)}`,
      'deactivateInactiveGraphPath',
    );
  }
}
