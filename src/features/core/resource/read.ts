import { useSyncExternalStore } from 'react';

import type { DeepReadonly } from '@/shared/types/deepReadonly';
import { useDocumentStateStore, type DocumentState } from './documentStateStore';
import { useResourceStore } from './resourceStore';
import type { ProjectResourceMeta, ResourceKey } from './resourceTypes';

export interface ResourceProjectionSnapshot {
  readonly resources: DeepReadonly<Record<ResourceKey, ProjectResourceMeta>>;
  readonly graphOrder: readonly string[];
  readonly documents: DeepReadonly<Record<ResourceKey, DocumentState>>;
}

export interface ResourceReadCapability {
  readonly getSnapshot: () => DeepReadonly<ResourceProjectionSnapshot>;
  readonly subscribe: (listener: () => void) => () => void;
}

function cloneAndFreeze<T>(value: T): T {
  if (Array.isArray(value)) return Object.freeze(value.map(cloneAndFreeze)) as T;
  if (value === null || typeof value !== 'object') return value;
  if (value instanceof Date) return Object.freeze(new Date(value.getTime())) as T;
  if (value instanceof Map) {
    return new Map(
      [...value.entries()].map(([key, nested]) => [
        cloneAndFreeze(key),
        cloneAndFreeze(nested),
      ]),
    ) as T;
  }
  if (value instanceof Set) return new Set([...value].map(cloneAndFreeze)) as T;
  return Object.freeze(Object.fromEntries(
    Object.entries(value as Record<string, unknown>)
      .map(([key, nested]) => [key, cloneAndFreeze(nested)]),
  )) as T;
}

function buildSnapshot(): DeepReadonly<ResourceProjectionSnapshot> {
  const resourceState = useResourceStore.getState();
  return Object.freeze({
    resources: cloneAndFreeze(resourceState.resources),
    graphOrder: cloneAndFreeze(resourceState.graphOrder),
    documents: cloneAndFreeze(useDocumentStateStore.getState().documents),
  });
}

let currentSnapshot = buildSnapshot();
const listeners = new Set<() => void>();

function refreshSnapshot(): void {
  currentSnapshot = buildSnapshot();
  for (const listener of listeners) listener();
}

useResourceStore.subscribe(refreshSnapshot);
useDocumentStateStore.subscribe(refreshSnapshot);

export function getResourceSnapshot(): DeepReadonly<ResourceProjectionSnapshot> {
  return currentSnapshot;
}

export function subscribeResourceRead(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useResourceRead<T>(
  selector: (snapshot: DeepReadonly<ResourceProjectionSnapshot>) => T,
): T {
  const snapshot = useSyncExternalStore(
    subscribeResourceRead,
    getResourceSnapshot,
    getResourceSnapshot,
  );
  return selector(snapshot);
}

export const resourceRead: ResourceReadCapability = {
  getSnapshot: getResourceSnapshot,
  subscribe: subscribeResourceRead,
};
