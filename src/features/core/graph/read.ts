import { createReadProjection, useReadProjection } from "@/features/core/state/readProjection";

import type { DeepReadonly } from "@/shared/types/deepReadonly";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import type { GraphEntityBucket } from "@/features/core/dataStore/graphEntityAccess";
import { useGraphMetaStore, type GraphMeta } from "@/features/core/dataStore/graphMetaStore";
import type { GraphPath } from "@/shared/types/domain/ids";

export interface GraphProjectionSnapshot {
  readonly graphEntities: DeepReadonly<Record<GraphPath, GraphEntityBucket>>;
  readonly graphMeta: DeepReadonly<Record<GraphPath, GraphMeta>>;
}

function buildSnapshot(): DeepReadonly<GraphProjectionSnapshot> {
  return {
    graphEntities: useGraphProjectionStore.getState().graphEntities,
    graphMeta: useGraphMetaStore.getState().graphs,
  };
}

const projection = createReadProjection(buildSnapshot, [
  useGraphProjectionStore,
  useGraphMetaStore,
]);
export const getGraphSnapshot = projection.getSnapshot;
export function useGraphRead<T>(
  selector: (snapshot: DeepReadonly<GraphProjectionSnapshot>) => T,
): T {
  return useReadProjection(projection, selector);
}
