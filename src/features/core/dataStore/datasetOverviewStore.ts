import { create } from 'zustand';
import { DatabaseId } from '@/shared/types/domain/ids';
import type { DatasetOverview } from '@/shared/types/domain/dataframe';
export type { DataCompleteness, DatasetOverview, SchemaOverview, SizeShape } from '@/shared/types/domain/dataframe';

interface DatasetOverviewStore {
  overviewByDatabase: Record<DatabaseId, DatasetOverview>;

  setOverview(dbId: DatabaseId, overview: DatasetOverview): void;
  clearOverview(dbId: DatabaseId): void;
  clear(): void;
}

export const useDatasetOverviewStore = create<DatasetOverviewStore>((set) => ({
  overviewByDatabase: {},

  setOverview: (dbId, overview) =>
    set((state) => ({
      overviewByDatabase: {
        ...state.overviewByDatabase,
        [dbId]: overview,
      },
    })),

  clearOverview: (dbId) =>
    set((state) => {
      const next = { ...state.overviewByDatabase };
      delete next[dbId];
      return { overviewByDatabase: next };
    }),

  clear: () => set({ overviewByDatabase: {} }),
}));
