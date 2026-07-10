import { useGraphDataStore, useVariableStore } from '@/features/core/dataStore';
import { useExecutionStore } from '@/features/core/execution';
import { getActiveLayoutTab } from '@/features/core/layout/layoutTabQueries';
import { isGraphTabDirty } from '@/features/core/layout/graphTabQueries';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { markResourceLoaded } from '@/features/core/resource';
import { releaseGraphViewport } from '@/features/core/viewport';
import { inferGraphResourceKind } from '@/shared/types/domain/graphResourcePath';
import { GraphService } from '@/services/graph/graphService';
import { logger } from '@/utils/appLogger';
import { enforceGraphDocumentCacheLimit } from './graphDocumentCachePolicy';

function resolveGroupGraphPath(groupId: string): string | null {
  const active = getActiveLayoutTab(groupId);
  if (!active?.tab) return null;
  if (active.tab.type === 'event' || active.tab.type === 'function') {
    return active.tab.id;
  }
  return null;
}

/** Unload frontend/backend graph cache when the path is not the focused session. */
export async function unloadGraphDocument(graphPath: string): Promise<void> {
  if (useGraphSessionStore.getState().isFocusedGraphPath(graphPath)) return;
  if (isGraphTabDirty(graphPath)) return;

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

/** Drop hydrated session for a non-focused editor group; tabs stay open for lazy reload. */
export async function suspendEditorGroupGraphSession(groupId: string): Promise<void> {
  const sessionStore = useGraphSessionStore.getState();
  const graphPath = resolveGroupGraphPath(groupId);

  sessionStore.clearFocusedSession(groupId);

  if (graphPath) {
    await unloadGraphDocument(graphPath);
    await enforceGraphDocumentCacheLimit();
  }
}
