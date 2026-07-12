import { useGraphDataStore } from './graphDataStore';
import { getDocumentState } from '@/features/core/resource';
import { inferGraphResourceKind } from '@/shared/types/domain/graphResourcePath';

/** True when graph body is in memory and does not need a backend reload. */
export function isGraphCachedInMemory(graphPath: string): boolean {
  if (!useGraphDataStore.getState().hasGraph(graphPath)) return false;

  const kind = inferGraphResourceKind(graphPath);
  if (!kind) return false;

  const doc = getDocumentState({ id: graphPath, kind });
  if (doc?.stale || doc?.conflict) return false;

  return true;
}
