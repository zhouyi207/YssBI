import { useGraphDataStore } from '@/features/core/dataStore';
import { shouldRetainGraphDocument } from './graphDocumentRetention';
import { unloadGraphDocument } from './graphDocumentUnload';

/** Max in-memory graph documents (VS Code-style cap for split-screen). */
export const MAX_HYDRATED_GRAPH_DOCUMENTS = 4;

const lastAccessAt = new Map<string, number>();

export function touchGraphDocument(graphPath: string): void {
  lastAccessAt.set(graphPath, Date.now());
}

/** Evict LRU hydrated graphs until within cap; skips paths covered by retention guards. */
export async function enforceGraphDocumentCacheLimit(): Promise<void> {
  const hydrated = Object.keys(useGraphDataStore.getState().graphEntities);
  if (hydrated.length <= MAX_HYDRATED_GRAPH_DOCUMENTS) return;

  const evictionOrder = hydrated
    .filter((path) => !shouldRetainGraphDocument(path))
    .sort((a, b) => (lastAccessAt.get(a) ?? 0) - (lastAccessAt.get(b) ?? 0));

  for (const path of evictionOrder) {
    if (Object.keys(useGraphDataStore.getState().graphEntities).length <= MAX_HYDRATED_GRAPH_DOCUMENTS) {
      break;
    }
    await unloadGraphDocument(path);
    lastAccessAt.delete(path);
  }
}
