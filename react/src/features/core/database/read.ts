import { createReadProjection, useReadProjection } from "@/features/core/state/readProjection";

import type { DeepReadonly } from "@/shared/types/deepReadonly";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import type { DatabaseRecord } from "@/shared/types/domain/database";

export interface DatabaseReadSnapshot {
  readonly databases: DeepReadonly<Record<string, DatabaseRecord>>;
}

export function selectDatabaseNames(snapshot: DatabaseReadSnapshot): Record<string, string> {
  return Object.fromEntries(
    Object.entries(snapshot.databases).map(([id, database]) => [id, database.name]),
  );
}

function buildSnapshot(): DeepReadonly<DatabaseReadSnapshot> {
  const state = useResourceStore.getState();
  return {
    databases: state.databases,
  };
}

const projection = createReadProjection(buildSnapshot, [useResourceStore]);
export function useDatabaseRead<T>(
  selector: (snapshot: DeepReadonly<DatabaseReadSnapshot>) => T,
): T {
  return useReadProjection(projection, selector);
}
