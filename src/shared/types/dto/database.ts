/**
 * IPC database names are retained here as a stable import surface. The
 * structural contract and normalization policy live in the domain owner so
 * Application, Core, and Views do not need to depend on this wire module.
 */
export type {
  ColumnInfo,
  DatabaseCellValue,
  DatabaseDecl,
  DatabaseDeclDTO,
  DatabaseDocumentDto,
  DatabaseEngineDTO,
  DatabaseEngineSqlDTO,
  DatabaseImportSourceDTO,
  DatabaseImportSqlEngineDTO,
  DatabaseRecord,
  DatabaseRow,
  CsvEngineConfig,
  DuckDbEngineConfig,
  ExcelEngineConfig,
  InMemoryEngineConfig,
  LoadDatabaseResult,
  ParquetEngineConfig,
  SqlEngineConfig,
} from '../domain/database';

export {
  databaseRecordFromLoad,
  databaseSourcePath,
  displayNameFromEngine,
  normalizeDatabaseRecord,
  normalizeDatabases,
} from '../domain/database';
