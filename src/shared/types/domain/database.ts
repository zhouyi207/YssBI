/**
 * Domain Types - Database
 * 
 * 数据库和数据源相关的类型定义
 */

// ==================== 数据库声明 ====================

export interface ColumnInfo {
  name: string;
  type: string;
}

export type DatabaseCellValue = string | number | boolean | null;
export type DatabaseRow = DatabaseCellValue[];

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

export type DatabaseEngineDTO =
  | { sql: SqlEngineConfig }
  | { csv: CsvEngineConfig }
  | { parquet: ParquetEngineConfig }
  | { excel: ExcelEngineConfig }
  | { duckDb: DuckDbEngineConfig }
  | { inMemory: InMemoryEngineConfig };

export type DatabaseImportSourceDTO =
  | { sql: { engine: DatabaseImportSqlEngineDTO; connectionString: string; table: string } }
  | { csv: CsvEngineConfig }
  | { parquet: ParquetEngineConfig }
  | { excel: ExcelEngineConfig };

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
  loadFailed?: boolean;
}

export type DatabaseDecl = DatabaseDeclDTO;

export interface DatabaseDocumentDto {
  id: string;
  engine: DatabaseEngineDTO;
  schemaVersion: number;
  required: boolean;
  name: string | null;
}

export type DatabaseRecord = DatabaseDeclDTO & { name: string };

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function normalizeColumns(raw: unknown): ColumnInfo[] | undefined {
  if (!Array.isArray(raw)) return undefined;
  const columns: ColumnInfo[] = [];
  for (const item of raw) {
    if (!isRecord(item)) continue;
    if (typeof item.name === 'string' && typeof item.type === 'string') {
      columns.push({ name: item.name, type: item.type });
    }
  }
  return columns.length > 0 ? columns : undefined;
}

function pathStem(path: string): string | undefined {
  const parts = path.replace(/\\/g, '/').split('/');
  const file = parts[parts.length - 1] || '';
  const stem = file.replace(/\.[^.]+$/, '');
  return stem || file || undefined;
}

export function displayNameFromEngine(
  engine: DatabaseEngineDTO | undefined,
): string | undefined {
  if (!engine) return undefined;
  if ('csv' in engine) return pathStem(engine.csv.path);
  if ('parquet' in engine) return pathStem(engine.parquet.path);
  if ('excel' in engine) return engine.excel.sheet || pathStem(engine.excel.path);
  if ('duckDb' in engine) return engine.duckDb.table || pathStem(engine.duckDb.path);
  if ('sql' in engine) return engine.sql.table;
  if ('inMemory' in engine) return engine.inMemory.name;
  return undefined;
}

export function databaseSourcePath(
  engine: DatabaseEngineDTO | undefined,
): string | undefined {
  if (!engine) return undefined;
  if ('csv' in engine) return engine.csv.path;
  if ('parquet' in engine) return engine.parquet.path;
  if ('excel' in engine) return engine.excel.path;
  if ('duckDb' in engine) return engine.duckDb.path;
  if ('sql' in engine) return engine.sql.connectionString;
  return undefined;
}

export function normalizeDatabaseRecord(
  id: string,
  raw: unknown,
  existing?: DatabaseRecord,
): DatabaseRecord {
  const input = isRecord(raw) ? raw : {};
  const engine = (input.engine as DatabaseEngineDTO | undefined) ?? existing?.engine;
  const name = typeof input.name === 'string' && input.name.trim()
    ? input.name.trim()
    : displayNameFromEngine(engine) ?? existing?.name ?? id;
  const columns = normalizeColumns(input.columns) ?? existing?.columns;
  const columnCount = typeof input.columnCount === 'number'
    ? input.columnCount
    : columns?.length ?? existing?.columnCount;
  return {
    id: typeof input.id === 'string' ? input.id : id,
    name,
    engine,
    schemaVersion: typeof input.schemaVersion === 'number'
      ? input.schemaVersion
      : existing?.schemaVersion ?? 0,
    required: typeof input.required === 'boolean'
      ? input.required
      : existing?.required ?? false,
    columns,
    rowCount: typeof input.rowCount === 'number' ? input.rowCount : existing?.rowCount,
    columnCount,
    loadFailed: typeof input.loadFailed === 'boolean'
      ? input.loadFailed
      : existing?.loadFailed ?? false,
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
  meta: Pick<DatabaseDeclDTO, 'id' | 'name' | 'rowCount' | 'columnCount' | 'columns'>,
  existing?: DatabaseRecord,
): DatabaseRecord {
  return normalizeDatabaseRecord(meta.id, meta, existing);
}

// ==================== 数据来源配置 ====================

/**
 * CSV 数据源
 */
export interface CsvSource {
    type: "csv";
    path: string;           // 文件路径（相对于项目目录）
    delimiter?: string;     // 分隔符
    encoding?: string;      // 编码
    has_header?: boolean;   // 是否有表头
}

/**
 * JSON 数据源
 */
export interface JsonSource {
    type: "json";
    path: string;           // 文件路径
}

/**
 * Excel 数据源
 */
export interface ExcelSource {
    type: "excel";
    path: string;           // 文件路径
    sheet?: string;         // 工作表名称
    start_row?: number;     // 起始行
}

/**
 * SQL 数据源
 */
export interface SqlSource {
    type: "sql";
    connection_id: string;              // 连接标识符
    query: string;                      // SQL 查询语句
    parameters?: Record<string, unknown>; // 查询参数
}

/**
 * API 数据源
 */
export interface ApiSource {
    type: "api";
    url: string;                        // 请求 URL
    method?: "GET" | "POST" | "PUT" | "DELETE"; // HTTP 方法
    headers?: Record<string, string>;   // 请求头
    body?: unknown;                     // 请求体
}

/**
 * 转换数据源
 */
export interface TransformSource {
    type: "transform";
    source_variable_id: string;         // 源变量 ID
    operations: TransformOperation[];   // 转换操作列表
}

/**
 * 内联数据源
 */
export interface InlineSource {
    type: "inline";
    data: unknown;                      // 内联数据
}

/**
 * 数据来源配置
 * 支持多种数据源类型
 */
export type DataSourceConfig =
    | CsvSource
    | JsonSource
    | ExcelSource
    | SqlSource
    | ApiSource
    | TransformSource
    | InlineSource;

// ==================== 转换操作 ====================

/**
 * 过滤操作
 */
export interface FilterOperation {
    op: "filter";
    expression: string;
}

/**
 * 选择列操作
 */
export interface SelectOperation {
    op: "select";
    columns: string[];
}

/**
 * 排序操作
 */
export interface SortOperation {
    op: "sort";
    column: string;
    descending?: boolean;
}

/**
 * 分组聚合操作
 */
export interface GroupByOperation {
    op: "group_by";
    columns: string[];
    aggregations: Aggregation[];
}

/**
 * 限制行数操作
 */
export interface LimitOperation {
    op: "limit";
    count: number;
}

/**
 * 表达式操作
 */
export interface ExpressionOperation {
    op: "expression";
    expr: string;
}

/**
 * 转换操作
 * 用于数据转换和处理
 */
export type TransformOperation =
    | FilterOperation
    | SelectOperation
    | SortOperation
    | GroupByOperation
    | LimitOperation
    | ExpressionOperation;

/**
 * 聚合函数
 */
export type AggregateFunction =
    | "sum"
    | "avg"
    | "min"
    | "max"
    | "count"
    | "first"
    | "last";

/**
 * 聚合操作
 */
export interface Aggregation {
    column: string;         // 源列
    function: AggregateFunction; // 聚合函数
    alias?: string;         // 结果别名
}
