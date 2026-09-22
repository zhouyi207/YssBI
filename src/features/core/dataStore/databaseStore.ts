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

  clear: () =>
    set({
      databases: {},
      revisions: {},
    }),
}));
