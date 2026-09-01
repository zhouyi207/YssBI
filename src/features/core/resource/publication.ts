import type { DeepReadonly } from "@/shared/types/deepReadonly";
import { useDocumentStateStore, type DocumentState } from "./documentStateStore";
import { useResourceStore } from "./resourceStore";
import type { ProjectResourceMeta, ResourceKey } from "./resourceTypes";
import type { ResourceProjectionSnapshot } from "./read";

export interface ResourceProjectionPublication {
  replaceSnapshot(snapshot: DeepReadonly<ResourceProjectionSnapshot>): void;
  clearForProject(projectInstanceId: string | null): void;
}

function cloneValue<T>(value: T): T {
  if (Array.isArray(value)) return value.map(cloneValue) as T;
  if (value === null || typeof value !== "object") return value;
  if (value instanceof Date) return new Date(value.getTime()) as T;
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

export function createResourceProjectionPublication(): ResourceProjectionPublication {
  return {
    replaceSnapshot: (snapshot) => {
      useResourceStore.setState({
        resources: cloneValue(snapshot.resources) as Record<ResourceKey, ProjectResourceMeta>,
        graphOrder: cloneValue(snapshot.graphOrder) as string[],
      });
      useDocumentStateStore.setState({
        documents: cloneValue(snapshot.documents) as Record<ResourceKey, DocumentState>,
      });
    },

    clearForProject: (_projectInstanceId) => {
      useResourceStore.setState({ resources: {}, graphOrder: [] });
      useDocumentStateStore.setState({ documents: {} });
    },
  };
}
