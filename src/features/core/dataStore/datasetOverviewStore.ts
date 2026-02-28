import { create } from 'zustand';
import { DatabaseId } from '@/shared/types/domain/ids';

export interface SizeShape {
  nRows: number;
  nColumns: number;
  memorySize: number;
  duplicatedRows: number;
}

export interface SchemaOverview {
  numericCols: number;
  categoricalCols: number;
  stringCols: number;
  datetimeCols: number;
  boolCols: number;
}

export interface DataCompleteness {
  totalNulls: number;
  nullRatio: number;
  colsWithNulls: number;
  rowsWithNulls: number;
}

export interface DatasetOverview {
  sizeShape: SizeShape;
  schemaOverview: SchemaOverview;
  dataCompleteness: DataCompleteness;
}

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
