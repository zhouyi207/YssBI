import { create } from "zustand";
import { DatabaseId } from "@/shared/types/domain/ids";
import type { DatabaseRecord } from "@/shared/types/domain/database";
import { logger } from "@/features/core/observability/logger";

export type { DatabaseRecord };

interface DatabaseStore {
  databases: Record<DatabaseId, DatabaseRecord>;
  revisions: Record<DatabaseId, number>;

  addDatabase(id: DatabaseId, db: DatabaseRecord): void;
  updateDatabase(id: DatabaseId, patch: Partial<DatabaseRecord>): void;
  deleteDatabase(id: DatabaseId): void;

  setDatabaseSnapshot(
    dbs: Record<DatabaseId, DatabaseRecord>,
    revisions: Record<DatabaseId, number>,
  ): void;
  setDatabases(dbs: Record<DatabaseId, DatabaseRecord>): void;
  clear(): void;
}

export const useDatabaseStore = create<DatabaseStore>((set) => ({
  databases: {},
  revisions: {},

  addDatabase: (id, db) =>
    set((state) => {
      if (state.databases[id]) {
        logger.data.warn(`addDatabase: id "${id}" already exists`, "DatabaseStore");
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
        logger.data.warn(`updateDatabase: id "${id}" not found`, "DatabaseStore");
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
        logger.data.warn(`deleteDatabase: id "${id}" not found`, "DatabaseStore");
        return state;
      }

      const next = { ...state.databases };
      delete next[id];

      const revisions = { ...state.revisions };
      delete revisions[id];
      return { databases: next, revisions };
    }),

  setDatabaseSnapshot: (dbs, revisions) =>
    set({
      databases: dbs ?? {},
      revisions: revisions ?? {},
    }),

  setDatabases: (dbs) => set({ databases: dbs ?? {} }),

  clear: () =>
    set({
      databases: {},
      revisions: {},
    }),
}));
