/**
 * 数据库 DTO（对齐 Rust `schema/database.rs`）
 * Store 与 IPC 边界统一经 `normalizeDatabaseRecord` / `normalizeDatabases` 灌入。
 */

export interface ColumnInfo {
  name: string;
  type: string;
}

/** Scalar cell from paginated database row queries (IPC boundary). */
export type DatabaseCellValue = string | number | boolean | null;
export type DatabaseRow = DatabaseCellValue[];

/** `load_database` / `get_database_meta` IPC response. */
export interface LoadDatabaseResult {
  id: string;
  name: string;
  rowCount: number;
  columnCount: number;
  columns: ColumnInfo[];
}

export type DatabaseEngineSqlDTO =
  | { sqlite: { autoCreate?: boolean } }
  | { postgres: { ssl?: boolean } }
  | { mysql: { charset?: string } };

export type DatabaseImportSqlEngineDTO = 'sqlite' | 'postgres' | 'mysql';

export type DatabaseImportSourceDTO =
  | {
      sql: {
        engine: DatabaseImportSqlEngineDTO;
        connectionString: string;
        table: string;
      };
    }
  | { csv: CsvEngineConfig }
  | { parquet: ParquetEngineConfig }
  | { excel: ExcelEngineConfig };

export type DatabaseEngineDTO =
  | { sql: SqlEngineConfig }
  | { csv: CsvEngineConfig }
  | { parquet: ParquetEngineConfig }
  | { excel: ExcelEngineConfig }
  | { duckDb: DuckDbEngineConfig }
  | { inMemory: InMemoryEngineConfig };

/** 各引擎分支配置（由 `DatabaseEngineDTO` 派生，避免与 service 层重复定义） */
export type SqlEngineConfig = {
  engine: DatabaseEngineSqlDTO;
  connectionString: string;
  table: string;
};
export type CsvEngineConfig = {
  path: string;
  delimiter?: string;
  hasHeader?: boolean;
  inferSchemaLength?: number;
};
export type ParquetEngineConfig = { path: string; columns?: string[] };
export type ExcelEngineConfig = { path: string; sheet: string };
export type DuckDbEngineConfig = { path: string; table: string };
export type InMemoryEngineConfig = { name: string };


/** 后端 `DatabaseDeclDTO` + 前端 store 富元数据（列统计、加载状态等） */
export interface DatabaseDeclDTO {
  id: string;
  resourcePath?: string;
  name?: string;
  engine?: DatabaseEngineDTO;
  schemaVersion?: number;
  required?: boolean;
  columns?: ColumnInfo[];
  rowCount?: number;
  columnCount?: number;
  /** 具体加载错误仅保留在后端，wire 只传递机器状态。 */
  loadFailed?: boolean;
}

/** Canonical `DatabaseDecl` carried by resource mutation patches. */
export interface DatabaseDocumentDto {
  id: string;
  engine: DatabaseEngineDTO;
  schemaVersion: number;
  required: boolean;
  name: string | null;
}

/** 规范化后的 store 记录：`name` 必有 */
export type DatabaseRecord = DatabaseDeclDTO & { name: string };

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function normalizeColumns(raw: unknown): ColumnInfo[] | undefined {
  if (!Array.isArray(raw)) return undefined;
  const columns: ColumnInfo[] = [];
  for (const item of raw) {
    if (!isRecord(item)) continue;
    const name = item.name;
    const type = item.type;
    if (typeof name === 'string' && typeof type === 'string') {
      columns.push({ name, type });
    }
  }
  return columns.length > 0 ? columns : undefined;
}

/** 从引擎配置推导显示名（文件 stem / 表名 / 内存名） */
export function displayNameFromEngine(engine: DatabaseEngineDTO | undefined): string | undefined {
  if (!engine) return undefined;
  if ('csv' in engine) return pathStem(engine.csv.path);
  if ('parquet' in engine) return pathStem(engine.parquet.path);
  if ('excel' in engine) return engine.excel.sheet || pathStem(engine.excel.path);
  if ('duckDb' in engine) return engine.duckDb.table || pathStem(engine.duckDb.path);
  if ('sql' in engine) return engine.sql.table;
  if ('inMemory' in engine) return engine.inMemory.name;
  return undefined;
}

/** 数据源路径（Detail 只读展示） */
export function databaseSourcePath(engine: DatabaseEngineDTO | undefined): string | undefined {
  if (!engine) return undefined;
  if ('csv' in engine) return engine.csv.path;
  if ('parquet' in engine) return engine.parquet.path;
  if ('excel' in engine) return engine.excel.path;
  if ('duckDb' in engine) return engine.duckDb.path;
  if ('sql' in engine) return engine.sql.connectionString;
  return undefined;
}

function pathStem(path: string): string | undefined {
  const parts = path.replace(/\\/g, '/').split('/');
  const file = parts[parts.length - 1] || '';
  const stem = file.replace(/\.[^.]+$/, '');
  return stem || file || undefined;
}

/** 单条记录在入库边界规范化（合并已有富元数据） */
export function normalizeDatabaseRecord(
  id: string,
  raw: unknown,
  existing?: DatabaseRecord,
): DatabaseRecord {
  const input = isRecord(raw) ? raw : {};
  const engine = (input.engine as DatabaseEngineDTO | undefined) ?? existing?.engine;
  const name =
    typeof input.name === 'string' && input.name.trim()
      ? input.name.trim()
      : displayNameFromEngine(engine) ?? existing?.name ?? id;

  const columns = normalizeColumns(input.columns) ?? existing?.columns;
  const rowCount = typeof input.rowCount === 'number' ? input.rowCount : existing?.rowCount;
  const columnCount =
    typeof input.columnCount === 'number'
      ? input.columnCount
      : columns?.length ?? existing?.columnCount;

  return {
    id: typeof input.id === 'string' ? input.id : id,
    name,
    engine,
    schemaVersion:
      typeof input.schemaVersion === 'number'
        ? input.schemaVersion
        : existing?.schemaVersion ?? 0,
    required:
      typeof input.required === 'boolean' ? input.required : existing?.required ?? false,
    columns,
    rowCount,
    columnCount,
    loadFailed:
      typeof input.loadFailed === 'boolean'
        ? input.loadFailed
        : existing?.loadFailed ?? false,
  };
}

/** 批量规范化（项目加载 / refresh） */
export function normalizeDatabases(
  dbs: Record<string, unknown>,
  existing: Record<string, DatabaseRecord> = {},
): Record<string, DatabaseRecord> {
  const result: Record<string, DatabaseRecord> = {};
  for (const [id, db] of Object.entries(dbs)) {
    result[id] = normalizeDatabaseRecord(id, db, existing[id]);
  }
  return result;
}

/** 将加载结果并入 store；若 mutation event 已投影 authoritative engine，则保留它。 */
export function databaseRecordFromLoad(
  meta: Pick<DatabaseDeclDTO, 'id' | 'name' | 'rowCount' | 'columnCount' | 'columns'>,
  existing?: DatabaseRecord,
): DatabaseRecord {
  return normalizeDatabaseRecord(meta.id, meta, existing);
}
