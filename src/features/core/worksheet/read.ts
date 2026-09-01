import { useSyncExternalStore } from "react";

import type { DeepReadonly } from "@/shared/types/deepReadonly";
import type { WorksheetDocument, WorksheetIndexEntry } from "@/shared/types/domain/worksheet";
import { getWorksheetSnapshot as getSnapshot, subscribeWorksheetRead } from "./publication";
import type { OptimisticOperationKey } from "./publication";

export interface WorksheetCommittedSnapshot {
  readonly index: DeepReadonly<readonly WorksheetIndexEntry[]>;
  readonly documents: DeepReadonly<Record<string, WorksheetDocument>>;
}

export interface PendingWorksheetSave extends OptimisticOperationKey {
  readonly draftFingerprint: string;
  readonly status: "pending" | "acknowledged" | "unknown";
}

export interface WorksheetReadSnapshot {
  readonly index: DeepReadonly<readonly WorksheetIndexEntry[]>;
  readonly documents: DeepReadonly<Record<string, WorksheetDocument>>;
  readonly draftsByPath: DeepReadonly<Record<string, WorksheetDocument>>;
  readonly dirtyByPath: Readonly<Record<string, boolean>>;
  readonly pendingSaveByPath: DeepReadonly<Record<string, Record<string, PendingWorksheetSave>>>;
}

export type ReadonlyWorksheetSnapshot = DeepReadonly<WorksheetReadSnapshot>;

export function getWorksheetSnapshot(): ReadonlyWorksheetSnapshot {
  return getSnapshot();
}

export function useWorksheetRead<T>(selector: (state: ReadonlyWorksheetSnapshot) => T): T {
  const snapshot = useSyncExternalStore(
    subscribeWorksheetRead,
    getWorksheetSnapshot,
    getWorksheetSnapshot,
  );
  return selector(snapshot);
}

export { subscribeWorksheetRead };
