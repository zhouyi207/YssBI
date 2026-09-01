import {
  useDatabaseRead,
  type DatabaseReadSnapshot,
} from "@/features/application/dataManagement/databaseRead";
import { useDatabaseUi } from "@/features/core/database/ui";
import type { DatabaseId } from "@/shared/types/domain/ids";

export interface DatabaseProjection {
  readonly databases: DatabaseReadSnapshot;
  readonly selectedDatabaseId: DatabaseId | null;
  readonly selectedDatabase: DatabaseReadSnapshot["databases"][DatabaseId] | null;
  readonly selectedQuery: string;
  readonly selectedPage: number;
}

export function useDatabaseProjection(): DatabaseProjection {
  const databaseSnapshot = useDatabaseRead((snapshot) => snapshot);
  const uiSnapshot = useDatabaseUi((snapshot) => snapshot);
  const selectedDatabaseId = uiSnapshot.selectedDatabaseId;

  return {
    databases: databaseSnapshot,
    selectedDatabaseId,
    selectedDatabase: selectedDatabaseId
      ? (databaseSnapshot.databases[selectedDatabaseId] ?? null)
      : null,
    selectedQuery: selectedDatabaseId ? (uiSnapshot.queryByDatabase[selectedDatabaseId] ?? "") : "",
    selectedPage: selectedDatabaseId ? (uiSnapshot.pageByDatabase[selectedDatabaseId] ?? 0) : 0,
  };
}
