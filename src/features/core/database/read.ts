import { createReadProjection, useReadProjection } from "@/features/core/state/readProjection";

import type { DeepReadonly } from "@/shared/types/deepReadonly";
import { useDatabaseStore } from "@/features/core/dataStore/databaseStore";
import type { DatabaseRecord } from "@/shared/types/domain/database";

export interface DatabaseReadSnapshot {
  readonly databases: DeepReadonly<Record<string, DatabaseRecord>>;
  readonly revisions: DeepReadonly<Record<string, number>>;
}

function buildSnapshot(): DeepReadonly<DatabaseReadSnapshot> {
  const state = useDatabaseStore.getState();
  return {
    databases: state.databases,
    revisions: state.revisions,
  };
}

const projection = createReadProjection(buildSnapshot, [useDatabaseStore]);
export const getDatabaseSnapshot = projection.getSnapshot;
export function useDatabaseRead<T>(
  selector: (snapshot: DeepReadonly<DatabaseReadSnapshot>) => T,
): T {
  return useReadProjection(projection, selector);
}
