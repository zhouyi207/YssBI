import { create } from 'zustand';
import { DatabaseId } from '@/shared/types/domain/ids';

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
        console.warn(`[DatabaseStore] addDatabase: id "${id}" already exists`);
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
        console.warn(`[DatabaseStore] updateDatabase: id "${id}" not found`);
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
        console.warn(`[DatabaseStore] deleteDatabase: id "${id}" not found`);
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
