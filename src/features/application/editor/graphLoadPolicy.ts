import { getGraphByPath, useGraphDataStore } from '@/features/core/dataStore';
import { getDocumentState, markResourceLoaded } from '@/features/core/resource';
import { ensureGraphViewport } from '@/features/core/viewport';
import { inferGraphResourceKind } from '@/shared/types/domain/graphResourcePath';

/** True when graph body is already in memory and does not need a backend reload. */
export function isGraphCachedInMemory(graphPath: string): boolean {
  if (!useGraphDataStore.getState().hasGraph(graphPath)) return false;

  const kind = inferGraphResourceKind(graphPath);
  if (!kind) return false;

  const doc = getDocumentState({ id: graphPath, kind });
  if (doc?.stale || doc?.conflict) return false;

  return true;
}

/** Activate viewport + loaded flag for a graph that is already in the frontend cache. */
export function activateCachedGraph(graphPath: string): boolean {
  if (!isGraphCachedInMemory(graphPath)) return false;

  const kind = inferGraphResourceKind(graphPath);
  if (kind) {
    markResourceLoaded({ id: graphPath, kind }, true);
  }

  ensureGraphViewport(graphPath);
  return true;
}
