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

export type DatabaseImportSqlEngineDTO = "sqlite" | "postgres" | "mysql";

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

/** Frontend database projection with a display name resolved by Application. */
export type DatabaseRecord = DatabaseDeclDTO & { name: string };

export function databaseSourcePath(engine: DatabaseEngineDTO | undefined): string | undefined {
  if (!engine) return undefined;
  if ("csv" in engine) return engine.csv.path;
  if ("parquet" in engine) return engine.parquet.path;
  if ("excel" in engine) return engine.excel.path;
  if ("duckDb" in engine) return engine.duckDb.path;
  if ("sql" in engine) return engine.sql.connectionString;
  return undefined;
}
