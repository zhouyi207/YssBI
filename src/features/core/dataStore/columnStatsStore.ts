import { create } from 'zustand';
import { DatabaseId } from '@/shared/types/domain/ids';

export interface NumericColumnStats {
  columnName: string;
  columnType: string;
  kind: 'numeric';
  count: number;
  nullCount: number;
  min: number | null;
  max: number | null;
  mean: number | null;
  median: number | null;
  std: number | null;
  variance: number | null;
}

export interface StringColumnStats {
  columnName: string;
  columnType: string;
  kind: 'string';
  count: number;
  nullCount: number;
  emptyCount: number;
  validRatio: number;
  unique: number;
  mode: string | null;
  modeCount: number;
}

export type ColumnStats = NumericColumnStats | StringColumnStats;

/** 按列名索引的统计信息 */
type ColumnStatsMap = Record<string, ColumnStats>;

interface ColumnStatsStore {
  /** databaseId -> { columnName -> ColumnStats } */
  statsByDatabase: Record<DatabaseId, ColumnStatsMap>;

  /** 设置某数据库所有列的统计 */
  setAllStats(dbId: DatabaseId, stats: ColumnStats[]): void;

  /** 设置某数据库单列的统计（局部更新） */
  setColumnStat(dbId: DatabaseId, columnName: string, stat: ColumnStats): void;

  /** 清除某数据库的统计 */
  clearStats(dbId: DatabaseId): void;

  /** 清除所有 */
  clear(): void;
}

export const useColumnStatsStore = create<ColumnStatsStore>((set) => ({
  statsByDatabase: {},

  setAllStats: (dbId, stats) =>
    set((state) => {
      const map: ColumnStatsMap = {};
      for (const s of stats) {
        map[s.columnName] = s;
      }
      return {
        statsByDatabase: {
          ...state.statsByDatabase,
          [dbId]: map,
        },
      };
    }),

  setColumnStat: (dbId, columnName, stat) =>
    set((state) => ({
      statsByDatabase: {
        ...state.statsByDatabase,
        [dbId]: {
          ...(state.statsByDatabase[dbId] ?? {}),
          [columnName]: stat,
        },
      },
    })),

  clearStats: (dbId) =>
    set((state) => {
      const next = { ...state.statsByDatabase };
      delete next[dbId];
      return { statsByDatabase: next };
    }),

  clear: () => set({ statsByDatabase: {} }),
}));
