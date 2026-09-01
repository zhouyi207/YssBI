import type { DeepReadonly } from "@/shared/types/deepReadonly";
import {
  useNodeCatalogStore,
  type CatalogRequestIdentity,
  type LocalizedCatalogResponse,
  type NodeCatalogState,
} from "./nodeCatalogStore";

type CatalogError = Parameters<NodeCatalogState["storeError"]>[1];

export interface NodeCatalogPublication {
  readonly publishResponse: (
    identity: DeepReadonly<CatalogRequestIdentity>,
    response: DeepReadonly<LocalizedCatalogResponse>,
  ) => boolean;
  readonly publishError: (
    identity: DeepReadonly<CatalogRequestIdentity>,
    error: DeepReadonly<CatalogError>,
  ) => boolean;
  readonly observeResourcePublication: (projectInstanceId: string, revision: number) => boolean;
  readonly clear: () => void;
}

function cloneValue<T>(value: T): T {
  if (Array.isArray(value)) return value.map(cloneValue) as T;
  if (value === null || typeof value !== "object") return value;
  if (value instanceof Map) {
    return new Map(
      [...value.entries()].map(([key, nested]) => [cloneValue(key), cloneValue(nested)]),
    ) as T;
  }
  if (value instanceof Set) return new Set([...value].map(cloneValue)) as T;
  return Object.fromEntries(
    Object.entries(value as Record<string, unknown>).map(([key, nested]) => [
      key,
      cloneValue(nested),
    ]),
  ) as T;
}

export function createNodeCatalogPublication(): NodeCatalogPublication {
  return {
    publishResponse: (identity, response) =>
      useNodeCatalogStore
        .getState()
        .storeResponse(
          cloneValue(identity) as CatalogRequestIdentity,
          cloneValue(response) as LocalizedCatalogResponse,
        ),
    publishError: (identity, error) =>
      useNodeCatalogStore
        .getState()
        .storeError(
          cloneValue(identity) as CatalogRequestIdentity,
          cloneValue(error) as CatalogError,
        ),
    observeResourcePublication: (projectInstanceId, revision) =>
      useNodeCatalogStore.getState().observeResourcePublication(projectInstanceId, revision),
    clear: () => useNodeCatalogStore.getState().clear(),
  };
}
