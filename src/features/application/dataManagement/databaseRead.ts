import { createReadProjection, useReadProjection } from "@/features/core/state/readProjection";

import type { DeepReadonly } from "@/shared/types/deepReadonly";
import {
  useColumnDistributionStore,
  type DistributionMap,
} from "@/features/core/dataStore/columnDistributionStore";
import {
  useColumnStatsStore,
  type ColumnStatsMap,
} from "@/features/core/dataStore/columnStatsStore";
import { useDatabaseStore } from "@/features/core/dataStore/databaseStore";
import { useDatasetOverviewStore } from "@/features/core/dataStore/datasetOverviewStore";
import type { DatabaseId } from "@/shared/types/domain/ids";
import type { DatasetOverview } from "@/shared/types/domain/dataframe";
import type { DatabaseRecord } from "@/shared/types/domain/database";

export interface DatabaseReadSnapshot {
  readonly databases: DeepReadonly<Record<DatabaseId, DatabaseRecord>>;
  readonly revisions: DeepReadonly<Record<DatabaseId, number>>;
  readonly statsByDatabase: DeepReadonly<Record<DatabaseId, ColumnStatsMap>>;
  readonly distByDatabase: DeepReadonly<Record<DatabaseId, DistributionMap>>;
  readonly overviewByDatabase: DeepReadonly<Record<DatabaseId, DatasetOverview>>;
}

function buildSnapshot(): DatabaseReadSnapshot {
  const databaseState = useDatabaseStore.getState();
  return {
    databases: databaseState.databases,
    revisions: databaseState.revisions,
    statsByDatabase: useColumnStatsStore.getState().statsByDatabase,
    distByDatabase: useColumnDistributionStore.getState().distByDatabase,
    overviewByDatabase: useDatasetOverviewStore.getState().overviewByDatabase,
  };
}

const projection = createReadProjection(buildSnapshot, [
  useDatabaseStore,
  useColumnStatsStore,
  useColumnDistributionStore,
  useDatasetOverviewStore,
]);
export const getDatabaseSnapshot = projection.getSnapshot;
export function useDatabaseRead<T>(selector: (snapshot: DatabaseReadSnapshot) => T): T {
  return useReadProjection(projection, selector);
}
