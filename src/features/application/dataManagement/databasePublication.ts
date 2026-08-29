import type { DeepReadonly } from '@/shared/types/deepReadonly';
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
import type { ColumnDistribution, ColumnStats, DatasetOverview } from '@/shared/types/domain/dataframe';
import type { DatabaseId } from '@/shared/types/domain/ids';
import type { DatabaseRecord } from '@/shared/types/domain/database';
import type { ErrorReference } from '@/features/application/errorReference';
import type { DatabaseReadSnapshot } from '@/features/application/dataManagement/databaseRead';

export interface DatabasePublicationCapability {
  readonly replaceSnapshot: (snapshot: DatabaseReadSnapshot) => void;
  readonly publishDatabase: (
    database: DeepReadonly<DatabaseRecord>,
    revision?: number,
  ) => void;
  readonly publishDatabaseRevision: (id: DatabaseId, revision: number) => void;
  readonly removeDatabase: (id: DatabaseId) => void;
  readonly publishColumnStats: (id: DatabaseId, stats: readonly ColumnStats[]) => void;
  readonly removeColumnStats: (id: DatabaseId) => void;
  readonly publishColumnDistribution: (
    id: DatabaseId,
    distributions: readonly ColumnDistribution[],
  ) => void;
  readonly removeColumnDistribution: (id: DatabaseId) => void;
  readonly publishDatasetOverview: (id: DatabaseId, overview: DeepReadonly<DatasetOverview>) => void;
  readonly removeDatasetOverview: (id: DatabaseId) => void;
  readonly publishDatabaseFailure: (id: DatabaseId, error: ErrorReference) => void;
  readonly clearForProject: () => void;
}

function clone<T>(value: T): T {
  return structuredClone(value) as unknown as T;
}

function toStatsMap(stats: readonly ColumnStats[]): ColumnStatsMap {
  return Object.fromEntries(
    stats.map((stat) => [stat.columnName, clone(stat) as ColumnStats]),
  ) as ColumnStatsMap;
}

function toDistributionMap(
  distributions: readonly ColumnDistribution[],
): DistributionMap {
  return Object.fromEntries(
    distributions.map((distribution) => [
      distribution.columnName,
      clone(distribution) as ColumnDistribution,
    ]),
  ) as DistributionMap;
}

export function createDatabasePublication(
  onFailure: (id: DatabaseId, error: ErrorReference) => void = () => undefined,
): DatabasePublicationCapability {
  return {
    replaceSnapshot: (snapshot) => {
      useDatabaseStore.setState({
        databases: clone(snapshot.databases) as unknown as Record<string, DatabaseRecord>,
        revisions: clone(snapshot.revisions) as unknown as Record<string, number>,
      });
      useColumnStatsStore.setState({
        statsByDatabase: clone(snapshot.statsByDatabase) as unknown as Record<string, ColumnStatsMap>,
      });
      useColumnDistributionStore.setState({
        distByDatabase: clone(snapshot.distByDatabase) as unknown as Record<string, DistributionMap>,
      });
      useDatasetOverviewStore.setState({
        overviewByDatabase: clone(snapshot.overviewByDatabase) as unknown as Record<string, DatasetOverview>,
      });
    },

    publishDatabase: (database, revision) => {
      useDatabaseStore.setState((state) => ({
        databases: {
          ...state.databases,
          [database.id]: clone(database) as unknown as DatabaseRecord,
        },
        revisions: revision === undefined
          ? state.revisions
          : { ...state.revisions, [database.id]: revision },
      }));
    },

    publishDatabaseRevision: (id, revision) => {
      useDatabaseStore.setState((state) => ({
        revisions: { ...state.revisions, [id]: revision },
      }));
    },

    removeDatabase: (id) => {
      useDatabaseStore.setState((state) => {
        const databases = { ...state.databases };
        const revisions = { ...state.revisions };
        delete databases[id];
        delete revisions[id];
        return { databases, revisions };
      });
      useColumnStatsStore.getState().clearStats(id);
      useColumnDistributionStore.getState().clearDistributions(id);
      useDatasetOverviewStore.getState().clearOverview(id);
    },

    publishColumnStats: (id, stats) => {
      useColumnStatsStore.setState((state) => ({
        statsByDatabase: { ...state.statsByDatabase, [id]: toStatsMap(stats) },
      }));
    },

    removeColumnStats: (id) => useColumnStatsStore.getState().clearStats(id),

    publishColumnDistribution: (id, distributions) => {
      useColumnDistributionStore.setState((state) => ({
        distByDatabase: { ...state.distByDatabase, [id]: toDistributionMap(distributions) },
      }));
    },

    removeColumnDistribution: (id) => useColumnDistributionStore.getState().clearDistributions(id),

    publishDatasetOverview: (id, overview) => {
      useDatasetOverviewStore.setState((state) => ({
        overviewByDatabase: { ...state.overviewByDatabase, [id]: clone(overview) },
      }));
    },

    removeDatasetOverview: (id) => useDatasetOverviewStore.getState().clearOverview(id),

    publishDatabaseFailure: (id, error) => onFailure(id, clone(error)),

    clearForProject: () => {
      useDatabaseStore.setState({ databases: {}, revisions: {} });
      useColumnStatsStore.setState({ statsByDatabase: {} });
      useColumnDistributionStore.setState({ distByDatabase: {} });
      useDatasetOverviewStore.setState({ overviewByDatabase: {} });
    },
  };
}
