import { createReadProjection, useReadProjection } from "@/features/core/state/readProjection";

import type { DeepReadonly } from "@/shared/types/deepReadonly";
import { useResourceStore, type DocumentState } from "./resourceStore";
import type { ProjectResourceMeta, ResourceKey } from "./resourceTypes";

export interface ResourceProjectionSnapshot {
  readonly resources: DeepReadonly<Record<ResourceKey, ProjectResourceMeta>>;
  readonly graphOrder: readonly string[];
  readonly documents: DeepReadonly<Record<ResourceKey, DocumentState>>;
}

function buildSnapshot(): DeepReadonly<ResourceProjectionSnapshot> {
  const resourceState = useResourceStore.getState();
  return {
    resources: resourceState.resources,
    graphOrder: resourceState.graphOrder,
    documents: resourceState.documents,
  };
}

const projection = createReadProjection(buildSnapshot, [useResourceStore]);
export const getResourceSnapshot = projection.getSnapshot;
export function useResourceRead<T>(
  selector: (snapshot: DeepReadonly<ResourceProjectionSnapshot>) => T,
): T {
  return useReadProjection(projection, selector);
}
