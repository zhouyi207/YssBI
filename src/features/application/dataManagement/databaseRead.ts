import { useSyncExternalStore } from 'react';

import type { DeepReadonly } from '@/shared/types/deepReadonly';
import { freezeProjectionSnapshot } from '@/shared/types/deepReadonly';
import {
  useColumnDistributionStore,
  type DistributionMap,
} from '@/features/core/dataStore/columnDistributionStore';
import {
  useColumnStatsStore,
  type ColumnStatsMap,
} from '@/features/core/dataStore/columnStatsStore';
import { useDatabaseStore } from '@/features/core/dataStore/databaseStore';
import { useDatasetOverviewStore } from '@/features/core/dataStore/datasetOverviewStore';
import type { DatabaseId } from '@/shared/types/domain/ids';
import type { DatasetOverview } from '@/shared/types/domain/dataframe';
import type { DatabaseRecord } from '@/shared/types/domain/database';

export interface DatabaseReadSnapshot {
  readonly databases: DeepReadonly<Record<DatabaseId, DatabaseRecord>>;
  readonly revisions: DeepReadonly<Record<DatabaseId, number>>;
  readonly statsByDatabase: DeepReadonly<Record<DatabaseId, ColumnStatsMap>>;
  readonly distByDatabase: DeepReadonly<Record<DatabaseId, DistributionMap>>;
  readonly overviewByDatabase: DeepReadonly<Record<DatabaseId, DatasetOverview>>;
}

export interface DatabaseReadCapability {
  readonly getSnapshot: () => DatabaseReadSnapshot;
  readonly subscribe: (listener: () => void) => () => void;
}

function buildSnapshot(): DatabaseReadSnapshot {
  const databaseState = useDatabaseStore.getState();
  return freezeProjectionSnapshot({
    databases: databaseState.databases,
    revisions: databaseState.revisions,
    statsByDatabase: useColumnStatsStore.getState().statsByDatabase,
    distByDatabase: useColumnDistributionStore.getState().distByDatabase,
    overviewByDatabase: useDatasetOverviewStore.getState().overviewByDatabase,
  });
}

let currentSnapshot = buildSnapshot();
const listeners = new Set<() => void>();

function refreshSnapshot(): void {
  currentSnapshot = buildSnapshot();
  for (const listener of listeners) listener();
}

useDatabaseStore.subscribe(refreshSnapshot);
useColumnStatsStore.subscribe(refreshSnapshot);
useColumnDistributionStore.subscribe(refreshSnapshot);
useDatasetOverviewStore.subscribe(refreshSnapshot);

export function getDatabaseSnapshot(): DatabaseReadSnapshot {
  return currentSnapshot;
}

export function subscribeDatabaseRead(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useDatabaseRead<T>(
  selector: (snapshot: DatabaseReadSnapshot) => T,
): T {
  const snapshot = useSyncExternalStore(
    subscribeDatabaseRead,
    getDatabaseSnapshot,
    getDatabaseSnapshot,
  );
  return selector(snapshot);
}

export const databaseRead: DatabaseReadCapability = {
  getSnapshot: getDatabaseSnapshot,
  subscribe: subscribeDatabaseRead,
};
