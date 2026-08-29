import { useDatabaseStore } from '@/features/core/dataStore/databaseStore';
import type { DatabaseRecord } from '@/shared/types/domain/database';

export const databasePublication = {
  updateDatabase(id: string, patch: Partial<DatabaseRecord>): void {
    useDatabaseStore.getState().updateDatabase(id, patch);
  },
};
