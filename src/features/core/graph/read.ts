import { useSyncExternalStore } from 'react';

import type { DeepReadonly } from '@/shared/types/deepReadonly';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import type { GraphEntityBucket } from '@/features/core/dataStore/graphEntityAccess';
import { useGraphMetaStore, type GraphMeta } from '@/features/core/dataStore/graphMetaStore';
import type { GraphPath } from '@/shared/types/domain/ids';

export interface GraphProjectionSnapshot {
  readonly graphEntities: DeepReadonly<Record<GraphPath, GraphEntityBucket>>;
  readonly graphMeta: DeepReadonly<Record<GraphPath, GraphMeta>>;
}

export interface GraphReadCapability {
  readonly getSnapshot: () => DeepReadonly<GraphProjectionSnapshot>;
  readonly subscribe: (listener: () => void) => () => void;
}

function cloneAndFreeze<T>(value: T): T {
  if (Array.isArray(value)) {
    return Object.freeze(value.map(cloneAndFreeze)) as T;
  }
  if (value === null || typeof value !== 'object') return value;
  if (value instanceof Date) {
    return Object.freeze(new Date(value.getTime())) as T;
  }
  if (value instanceof Map) {
    return new Map(
      [...value.entries()].map(([key, nested]) => [
        cloneAndFreeze(key),
        cloneAndFreeze(nested),
      ]),
    ) as T;
  }
  if (value instanceof Set) {
    return new Set([...value].map(cloneAndFreeze)) as T;
  }
  const copy = Object.fromEntries(
    Object.entries(value as Record<string, unknown>)
      .map(([key, nested]) => [key, cloneAndFreeze(nested)]),
  );
  return Object.freeze(copy) as T;
}

function buildSnapshot(): DeepReadonly<GraphProjectionSnapshot> {
  return Object.freeze({
    graphEntities: cloneAndFreeze(useGraphDataStore.getState().graphEntities),
    graphMeta: cloneAndFreeze(useGraphMetaStore.getState().graphs),
  });
}

let currentSnapshot = buildSnapshot();
const listeners = new Set<() => void>();

function refreshSnapshot(): void {
  currentSnapshot = buildSnapshot();
  for (const listener of listeners) listener();
}

useGraphDataStore.subscribe(refreshSnapshot);
useGraphMetaStore.subscribe(refreshSnapshot);

export function getGraphSnapshot(): DeepReadonly<GraphProjectionSnapshot> {
  return currentSnapshot;
}

export function subscribeGraphRead(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useGraphRead<T>(
  selector: (snapshot: DeepReadonly<GraphProjectionSnapshot>) => T,
): T {
  const snapshot = useSyncExternalStore(
    subscribeGraphRead,
    getGraphSnapshot,
    getGraphSnapshot,
  );
  return selector(snapshot);
}

export const graphRead: GraphReadCapability = {
  getSnapshot: getGraphSnapshot,
  subscribe: subscribeGraphRead,
};
