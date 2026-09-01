import { useSyncExternalStore } from "react";

import type { DeepReadonly } from "@/shared/types/deepReadonly";
import { useNodeCatalogTreeStore } from "./nodeCatalogTreeStore";

export interface NodeCatalogUiSnapshot {
  readonly scopeKey: string | null;
  readonly query: string;
  readonly expandedCategoryIds: ReadonlySet<string>;
}

export interface NodeCatalogUiCapability {
  readonly getSnapshot: () => DeepReadonly<NodeCatalogUiSnapshot>;
  readonly subscribe: (listener: () => void) => () => void;
  readonly setScope: (scopeKey: string | null) => void;
  readonly setQuery: (query: string) => void;
  readonly setCategoryExpanded: (categoryId: string, expanded: boolean) => void;
  readonly setCategoriesExpanded: (categoryIds: Iterable<string>, expanded: boolean) => void;
  readonly reset: () => void;
}

function buildSnapshot(): DeepReadonly<NodeCatalogUiSnapshot> {
  const state = useNodeCatalogTreeStore.getState();
  return Object.freeze({
    scopeKey: state.scopeKey,
    query: state.query,
    expandedCategoryIds: new Set(state.expandedCategoryIds),
  });
}

let currentSnapshot = buildSnapshot();
const listeners = new Set<() => void>();

function refreshSnapshot(): void {
  currentSnapshot = buildSnapshot();
  for (const listener of listeners) listener();
}

useNodeCatalogTreeStore.subscribe(refreshSnapshot);

export function getNodeCatalogUiSnapshot(): DeepReadonly<NodeCatalogUiSnapshot> {
  return currentSnapshot;
}

export function subscribeNodeCatalogUi(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useNodeCatalogUi<T>(
  selector: (snapshot: DeepReadonly<NodeCatalogUiSnapshot>) => T,
): T {
  const snapshot = useSyncExternalStore(
    subscribeNodeCatalogUi,
    getNodeCatalogUiSnapshot,
    getNodeCatalogUiSnapshot,
  );
  return selector(snapshot);
}

export const nodeCatalogUi: NodeCatalogUiCapability = {
  getSnapshot: getNodeCatalogUiSnapshot,
  subscribe: subscribeNodeCatalogUi,
  setScope: (scopeKey) => useNodeCatalogTreeStore.getState().setScope(scopeKey),
  setQuery: (query) => useNodeCatalogTreeStore.getState().setQuery(query),
  setCategoryExpanded: (categoryId, expanded) =>
    useNodeCatalogTreeStore.getState().setCategoryExpanded(categoryId, expanded),
  setCategoriesExpanded: (categoryIds, expanded) =>
    useNodeCatalogTreeStore.getState().setCategoriesExpanded(categoryIds, expanded),
  reset: () => useNodeCatalogTreeStore.getState().reset(),
};
