import { DatabaseService } from "@/services/database/databaseService";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { logger } from "@/features/application/observability/appLogger";
import { databasePublication } from "@/features/core/database/publication";
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
      columns.push({ name: item.name, type: item.type });
    }
  }
  return columns.length > 0 ? columns : undefined;
}

function pathStem(path: string): string | undefined {
  const parts = path.replace(/\\/g, "/").split("/");
  const file = parts[parts.length - 1] || "";
  const stem = file.replace(/\.[^.]+$/, "");
  return stem || file || undefined;
}

export function displayNameFromEngine(engine: DatabaseEngine | undefined): string | undefined {
  if (!engine) return undefined;
  if ("csv" in engine) return pathStem(engine.csv.path);
  if ("parquet" in engine) return pathStem(engine.parquet.path);
  if ("excel" in engine) return engine.excel.sheet || pathStem(engine.excel.path);
  if ("duckDb" in engine) return engine.duckDb.table || pathStem(engine.duckDb.path);
  if ("sql" in engine) return engine.sql.table;
  if ("inMemory" in engine) return engine.inMemory.name;
  return undefined;
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
      : (displayNameFromEngine(engine) ?? existing?.name ?? id);
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
    databasePublication.updateDatabase(id, {
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
