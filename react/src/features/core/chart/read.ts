import { useResourceStore } from "@/features/core/resource/resourceStore";
import type { DeepReadonly } from "@/shared/types/deepReadonly";
import type { ChartDocument } from "@/shared/types/domain/chart";
import { createReadProjection, useReadProjection } from "@/features/core/state/readProjection";

export interface ChartReadSnapshot {
  readonly documents: DeepReadonly<Record<string, ChartDocument>>;
}

export type ReadonlyChartSnapshot = DeepReadonly<ChartReadSnapshot>;

const projection = createReadProjection(
  () => ({ documents: useResourceStore.getState().chartDocuments }),
  [useResourceStore],
);

export function useChartRead<T>(selector: (state: ReadonlyChartSnapshot) => T): T {
  return useReadProjection(projection, selector);
}
