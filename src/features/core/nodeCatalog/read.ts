import { useSyncExternalStore } from "react";

import type { DeepReadonly } from "@/shared/types/deepReadonly";
import {
  useNodeCatalogStore,
  type CatalogRequestState,
  type LocalizedCatalogResponse,
} from "./nodeCatalogStore";

export interface NodeCatalogProjectionSnapshot {
  readonly responses: DeepReadonly<Record<string, LocalizedCatalogResponse>>;
  readonly requests: DeepReadonly<Record<string, CatalogRequestState>>;
  readonly projectWatermarks: DeepReadonly<Record<string, number>>;
}

export interface NodeCatalogReadCapability {
  readonly getSnapshot: () => DeepReadonly<NodeCatalogProjectionSnapshot>;
  readonly subscribe: (listener: () => void) => () => void;
}

function cloneAndFreeze<T>(value: T): T {
  if (Array.isArray(value)) return Object.freeze(value.map(cloneAndFreeze)) as T;
  if (value === null || typeof value !== "object") return value;
  if (value instanceof Map) {
    return new Map(
      [...value.entries()].map(([key, nested]) => [cloneAndFreeze(key), cloneAndFreeze(nested)]),
    ) as T;
  }
  if (value instanceof Set) return new Set([...value].map(cloneAndFreeze)) as T;
  return Object.freeze(
    Object.fromEntries(
      Object.entries(value as Record<string, unknown>).map(([key, nested]) => [
        key,
        cloneAndFreeze(nested),
      ]),
    ),
  ) as T;
}

function buildSnapshot(): DeepReadonly<NodeCatalogProjectionSnapshot> {
  const state = useNodeCatalogStore.getState();
  return Object.freeze({
    responses: cloneAndFreeze(state.responses),
    requests: cloneAndFreeze(state.requests),
    projectWatermarks: cloneAndFreeze(state.projectWatermarks),
  });
}

let currentSnapshot = buildSnapshot();
const listeners = new Set<() => void>();

function refreshSnapshot(): void {
  currentSnapshot = buildSnapshot();
  for (const listener of listeners) listener();
}

useNodeCatalogStore.subscribe(refreshSnapshot);

export function getNodeCatalogSnapshot(): DeepReadonly<NodeCatalogProjectionSnapshot> {
  return currentSnapshot;
}

export function subscribeNodeCatalogRead(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useNodeCatalogRead<T>(
  selector: (snapshot: DeepReadonly<NodeCatalogProjectionSnapshot>) => T,
): T {
  const snapshot = useSyncExternalStore(
    subscribeNodeCatalogRead,
    getNodeCatalogSnapshot,
    getNodeCatalogSnapshot,
  );
  return selector(snapshot);
}

export const nodeCatalogRead: NodeCatalogReadCapability = {
  getSnapshot: getNodeCatalogSnapshot,
  subscribe: subscribeNodeCatalogRead,
};
