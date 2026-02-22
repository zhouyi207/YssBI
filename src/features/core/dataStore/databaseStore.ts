import { create } from 'zustand';
import { DatabaseId } from '@/shared/types/domain/ids';
import { logger } from '@/utils/appLogger';

/** 数据库/数据帧记录（支持 DatabaseDecl 及 DataFrame 等扩展字段） */
export type DatabaseRecord = Record<string, unknown>;

interface DatabaseStore {
  databases: Record<DatabaseId, DatabaseRecord>;

  // CRUD
  addDatabase(id: DatabaseId, db: DatabaseRecord): void;
  updateDatabase(id: DatabaseId, patch: Partial<DatabaseRecord>): void;
  deleteDatabase(id: DatabaseId): void;

  // 项目级
  setDatabases(dbs: Record<DatabaseId, DatabaseRecord>): void;
  clear(): void;
}

export const useDatabaseStore = create<DatabaseStore>((set) => ({
  // ======================
  // State
  // ======================
  databases: {},

  // ======================
  // CRUD
  // ======================
  addDatabase: (id, db) =>
    set((state) => {
      if (state.databases[id]) {
        logger.data.warn(`addDatabase: id "${id}" already exists`, 'DatabaseStore');
        return state;
      }

      return {
        databases: {
          ...state.databases,
          [id]: db,
        },
      };
    }),

  updateDatabase: (id, patch) =>
    set((state) => {
      const prev = state.databases[id];
      if (!prev) {
        logger.data.warn(`updateDatabase: id "${id}" not found`, 'DatabaseStore');
        return state;
      }

      return {
        databases: {
          ...state.databases,
          [id]: {
            ...prev,
            ...patch,
          },
        },
      };
    }),

  deleteDatabase: (id) =>
    set((state) => {
      if (!state.databases[id]) {
        logger.data.warn(`deleteDatabase: id "${id}" not found`, 'DatabaseStore');
        return state;
      }

      const next = { ...state.databases };
      delete next[id];

      return { databases: next };
    }),

  // ======================
  // Project-level
  // ======================
  setDatabases: (dbs) =>
    set({
      databases: dbs ?? {},
    }),

  clear: () =>
    set({
      databases: {},
    }),
}));
