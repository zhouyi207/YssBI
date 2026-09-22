import { DatabaseService } from "@/services/database/databaseService";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { logger } from "@/features/application/observability/appLogger";
import { useDatabaseStore } from "@/features/core/dataStore/databaseStore";
import { isColumnSemantic } from "@/shared/types/domain/database";
import type {
  ColumnInfo as DatabaseColumn,
  DatabaseEngineDTO as DatabaseEngine,
  DatabaseDeclDTO,
  DatabaseRecord,
} from "@/shared/types/domain/database";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function normalizeColumns(raw: unknown): DatabaseColumn[] | undefined {
  if (!Array.isArray(raw)) return undefined;
  const columns: DatabaseColumn[] = [];
  for (const item of raw) {
    if (!isRecord(item)) continue;
    if (typeof item.name === "string" && typeof item.type === "string") {
      if (item.semantic != null && !isColumnSemantic(item.semantic)) {
        throw new TypeError("Invalid column semantic metadata");
      }
      columns.push({
        name: item.name,
        type: item.type,
        ...(typeof item.physical === "string" ? { physical: item.physical } : {}),
        ...(item.semantic != null ? { semantic: item.semantic } : {}),
      });
    }
  }
  return columns.length > 0 ? columns : undefined;
}

export function normalizeDatabaseRecord(
  id: string,
  raw: unknown,
  existing?: DatabaseRecord,
): DatabaseRecord {
  const input = isRecord(raw) ? raw : {};
  const engine = (input.engine as DatabaseEngine | undefined) ?? existing?.engine;
  const name =
    typeof input.name === "string" && input.name.trim()
      ? input.name.trim()
      : (existing?.name ?? id);
  const columns = normalizeColumns(input.columns) ?? existing?.columns;
  const rowCount = typeof input.rowCount === "number" ? input.rowCount : existing?.rowCount;
  const columnCount =
    typeof input.columnCount === "number"
      ? input.columnCount
      : (columns?.length ?? existing?.columnCount);

  return {
    id: typeof input.id === "string" ? input.id : id,
    name,
    engine,
    schemaVersion:
      typeof input.schemaVersion === "number"
        ? input.schemaVersion
        : (existing?.schemaVersion ?? 0),
    required: typeof input.required === "boolean" ? input.required : (existing?.required ?? false),
    columns,
    rowCount,
    columnCount,
    loadFailed:
      typeof input.loadFailed === "boolean" ? input.loadFailed : (existing?.loadFailed ?? false),
  };
}

export function normalizeDatabases(
  databases: Record<string, unknown>,
  existing: Record<string, DatabaseRecord> = {},
): Record<string, DatabaseRecord> {
  return Object.fromEntries(
    Object.entries(databases).map(([id, database]) => [
      id,
      normalizeDatabaseRecord(id, database, existing[id]),
    ]),
  );
}

export function databaseRecordFromLoad(
  meta: Pick<DatabaseDeclDTO, "id" | "name" | "rowCount" | "columnCount" | "columns">,
  existing?: DatabaseRecord,
): DatabaseRecord {
  return normalizeDatabaseRecord(meta.id, meta, existing);
}

/** Hydrate missing read-only metadata without allowing stale windows to write. */
export async function hydrateDatabaseEditorMetadata(
  id: string,
  isCancelled: () => boolean = () => false,
): Promise<void> {
  const identity = captureProjectIdentity();
  try {
    const meta = await DatabaseService.getDatabaseMeta(identity.projectInstanceId, id);
    if (isCancelled() || !isCurrentProjectIdentity(identity)) return;
    useDatabaseStore.getState().updateDatabase(id, {
      name: meta.name,
      columns: meta.columns,
      rowCount: meta.rowCount,
      columnCount: meta.columnCount,
    });
  } catch (error) {
    if (!isCancelled() && isCurrentProjectIdentity(identity)) {
      logger.data.warn("getDatabaseMeta failed: " + String(error), "DatabaseEditorWindow");
    }
  }
}
