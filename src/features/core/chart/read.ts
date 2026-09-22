import type { DeepReadonly } from "@/shared/types/deepReadonly";
import type { ChartDocument } from "@/shared/types/domain/chart";
import { useChartDocumentStore } from "./chartDocumentStore";

export interface ChartReadSnapshot {
  readonly documents: DeepReadonly<Record<string, ChartDocument>>;
}

export type ReadonlyChartSnapshot = DeepReadonly<ChartReadSnapshot>;

export function useChartRead<T>(selector: (state: ReadonlyChartSnapshot) => T): T {
  return useChartDocumentStore((state) => selector({ documents: state.documents }));
}
