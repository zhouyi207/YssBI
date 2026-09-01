import { useSyncExternalStore } from "react";

import type { DeepReadonly } from "@/shared/types/deepReadonly";
import type { ChartDocument, ChartIndexEntry } from "@/shared/types/domain/chart";
import { getChartSnapshot as getSnapshot, subscribeChartRead } from "./publication";
import type { OptimisticOperationKey } from "./publication";

export interface ChartCommittedSnapshot {
  readonly index: DeepReadonly<readonly ChartIndexEntry[]>;
  readonly documents: DeepReadonly<Record<string, ChartDocument>>;
}

export interface PendingChartSave extends OptimisticOperationKey {
  readonly draftFingerprint: string;
  readonly status: "pending" | "acknowledged" | "unknown";
}

export interface ChartReadSnapshot {
  readonly index: DeepReadonly<readonly ChartIndexEntry[]>;
  readonly documents: DeepReadonly<Record<string, ChartDocument>>;
  readonly draftsByPath: DeepReadonly<Record<string, ChartDocument>>;
  readonly dirtyByPath: Readonly<Record<string, boolean>>;
  readonly pendingSaveByPath: DeepReadonly<Record<string, Record<string, PendingChartSave>>>;
}

export type ReadonlyChartSnapshot = DeepReadonly<ChartReadSnapshot>;

export function getChartSnapshot(): ReadonlyChartSnapshot {
  return getSnapshot();
}

export function useChartRead<T>(selector: (state: ReadonlyChartSnapshot) => T): T {
  const snapshot = useSyncExternalStore(subscribeChartRead, getChartSnapshot, getChartSnapshot);
  return selector(snapshot);
}

export { subscribeChartRead };
