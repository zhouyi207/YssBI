import { useGraphDataStore } from '@/features/core/dataStore';
import { isGraphTabDirty } from '@/features/core/layout/graphTabQueries';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { unloadGraphDocument } from './graphSessionLifecycle';

/** Max in-memory graph documents (VS Code-style cap for split-screen). */
export const MAX_HYDRATED_GRAPH_DOCUMENTS = 4;

const lastAccessAt = new Map<string, number>();

export function touchGraphDocument(graphPath: string): void {
  lastAccessAt.set(graphPath, Date.now());
}

function protectedGraphPaths(): Set<string> {
  const focused = useGraphSessionStore.getState().getFocusedGraphPath();
  return focused ? new Set([focused]) : new Set();
}

/** Evict LRU hydrated graphs until within cap; skips focused session and dirty paths. */
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
    if (protectedPaths.has(path)) continue;
    if (isGraphTabDirty(path)) continue;
    await unloadGraphDocument(path);
    lastAccessAt.delete(path);
  }
}
