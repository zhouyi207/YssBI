import { create } from 'zustand';
import { DatabaseId } from '@/shared/types/domain/ids';
import type { ColumnDistribution } from '@/shared/types/domain/dataframe';
export type {
  CategoryCount,
  ColumnDistribution,
  HistogramBin,
  NumericDistribution,
  StringDistribution,
} from '@/shared/types/domain/dataframe';

type DistributionMap = Record<string, ColumnDistribution>;

interface ColumnDistributionStore {
  distByDatabase: Record<DatabaseId, DistributionMap>;

  setAllDistributions(dbId: DatabaseId, dists: ColumnDistribution[]): void;
  setColumnDistribution(dbId: DatabaseId, columnName: string, dist: ColumnDistribution): void;
  clearDistributions(dbId: DatabaseId): void;
  clear(): void;
}

export const useColumnDistributionStore = create<ColumnDistributionStore>((set) => ({
  distByDatabase: {},

  setAllDistributions: (dbId, dists) =>
    set((state) => {
      const map: DistributionMap = {};
      for (const d of dists) {
        map[d.columnName] = d;
      }
      return {
        distByDatabase: {
          ...state.distByDatabase,
          [dbId]: map,
        },
      };
    }),

  setColumnDistribution: (dbId, columnName, dist) =>
    set((state) => ({
      distByDatabase: {
        ...state.distByDatabase,
        [dbId]: {
          ...(state.distByDatabase[dbId] ?? {}),
          [columnName]: dist,
        },
      },
    })),

  clearDistributions: (dbId) =>
    set((state) => {
      const next = { ...state.distByDatabase };
      delete next[dbId];
      return { distByDatabase: next };
    }),

  clear: () => set({ distByDatabase: {} }),
}));
