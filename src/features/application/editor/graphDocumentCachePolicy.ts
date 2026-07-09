import { useGraphDataStore } from '@/features/core/dataStore';
import { isGraphOpenInAnyTab } from '@/features/core/layout/graphTabQueries';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { deactivateInactiveGraphPath } from './deactivateInactiveGraphPath';

/** Max in-memory graph documents (VS Code-style cap for split-screen). */
export const MAX_HYDRATED_GRAPH_DOCUMENTS = 4;

const lastAccessAt = new Map<string, number>();

export function touchGraphDocument(graphPath: string): void {
  lastAccessAt.set(graphPath, Date.now());
}

function protectedGraphPaths(): Set<string> {
  const protectedPaths = new Set<string>();
  for (const path of Object.values(useGraphSessionStore.getState().activePathByGroup)) {
    if (path) protectedPaths.add(path);
  }
  return protectedPaths;
}

/** Evict LRU hydrated graphs until within cap; skips active tabs and dirty paths. */
export async function enforceGraphDocumentCacheLimit(): Promise<void> {
  const protectedPaths = protectedGraphPaths();
  let hydrated = Object.keys(useGraphDataStore.getState().graphEntities);

  if (hydrated.length <= MAX_HYDRATED_GRAPH_DOCUMENTS) return;

  const evictionOrder = hydrated
    .filter((path) => !protectedPaths.has(path))
    .sort((a, b) => (lastAccessAt.get(a) ?? 0) - (lastAccessAt.get(b) ?? 0));

  for (const path of evictionOrder) {
    if (Object.keys(useGraphDataStore.getState().graphEntities).length <= MAX_HYDRATED_GRAPH_DOCUMENTS) {
      break;
    }
    if (isGraphOpenInAnyTab(path)) continue;
    await deactivateInactiveGraphPath(path);
    lastAccessAt.delete(path);
  }
}
