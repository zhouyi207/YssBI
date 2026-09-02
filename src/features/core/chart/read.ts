import type { DeepReadonly } from "@/shared/types/deepReadonly";
import type { ChartDocument, ChartIndexEntry } from "@/shared/types/domain/chart";
import { useChartDocumentStore } from "./chartDocumentStore";

export interface ChartReadSnapshot {
  readonly index: DeepReadonly<readonly ChartIndexEntry[]>;
  readonly documents: DeepReadonly<Record<string, ChartDocument>>;
}

export type ReadonlyChartSnapshot = DeepReadonly<ChartReadSnapshot>;

export function getChartSnapshot(): ReadonlyChartSnapshot {
  const state = useChartDocumentStore.getState();
  return { index: state.index, documents: state.documents };
}

export function useChartRead<T>(selector: (state: ReadonlyChartSnapshot) => T): T {
  return useChartDocumentStore((state) =>
    selector({ index: state.index, documents: state.documents }),
  );
}
