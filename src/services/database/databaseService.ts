import { invokeCommand } from "@/services/ipc";
import type { ColumnSemantic } from "@/shared/types/domain/database";
import type { ColumnDistribution, EditState } from "@/shared/types/domain/dataframe";
import type {
  DatabaseImportSourceDTO,
  DatabaseRow,
  LoadDatabaseResult,
  SampleDatasetSummary,
} from "@/shared/types/dto/database";
import type { ResourceMutationResultDto } from "@/shared/types/dto/editorMutation";
import {
  databaseColumnValuesSchema,
  databaseDistributionsSchema,
  databaseEditStateSchema,
  databaseEmptyResultSchema,
  databaseMetadataSchema,
  databaseRowsSchema,
  databaseSourceEntriesSchema,
  parseDatabaseMutationResult,
} from "./databaseWireParser";

export type { DatabaseImportSourceDTO } from "@/shared/types/dto/database";

/** 分页行数据（含稳定 rowIds） */
export interface DatabaseRowsResult {
  rows: DatabaseRow[];
  rowIds: string[];
}

export interface DatabaseMutationCommandResult<T> {
  data: T;
  mutation: ResourceMutationResultDto;
}

/**
 * Database Service
 * 数据库服务 - 封装 load_database、delete_database、get_database_rows
 */
export class DatabaseService {
  static async listSampleDatasets(): Promise<SampleDatasetSummary[]> {
    const payload = await invokeCommand<unknown>("list_sample_datasets");
    if (!Array.isArray(payload) || payload.length > 64) {
      throw new TypeError("Invalid sample catalog response");
    }
    const ids = new Set<string>();
    for (const sample of payload) {
      if (
        !sample ||
        typeof sample !== "object" ||
        typeof sample.id !== "string" ||
        !/^[a-z0-9-]{1,64}$/.test(sample.id) ||
        ids.has(sample.id) ||
        typeof sample.name !== "string" ||
        !sample.name.trim() ||
        sample.name.length > 128 ||
        !Number.isInteger(sample.version) ||
        sample.version < 1 ||
        sample.version > 0xffffffff ||
        !Number.isSafeInteger(sample.rowCount) ||
        sample.rowCount < 0 ||
        sample.rowCount > 5_000_000 ||
        !Number.isInteger(sample.columnCount) ||
        sample.columnCount < 1 ||
        sample.columnCount > 512 ||
        !Number.isSafeInteger(sample.byteSize) ||
        sample.byteSize < 1 ||
        sample.byteSize > 64 * 1024 * 1024 ||
        Object.keys(sample).length !== 6
      ) {
        throw new TypeError("Invalid sample catalog response");
      }
      ids.add(sample.id);
    }
    return payload as SampleDatasetSummary[];
  }

  static async importSampleDataset(
    projectInstanceId: string,
    operationId: string,
    sampleId: string,
    version: number,
  ): Promise<DatabaseMutationCommandResult<LoadDatabaseResult>> {
    return parseDatabaseMutationResult(
      await invokeCommand<unknown>("import_sample_dataset", {
        projectInstanceId,
        operationId,
        sampleId,
        version,
      }),
      databaseMetadataSchema,
    );
  }

  /**
   * 加载数据库（CSV、Parquet 等）
   */
  static async loadDatabase(
    projectInstanceId: string,
    operationId: string,
    engine: DatabaseImportSourceDTO,
  ): Promise<DatabaseMutationCommandResult<LoadDatabaseResult>> {
    return parseDatabaseMutationResult(
      await invokeCommand<unknown>("load_database", { projectInstanceId, operationId, engine }),
      databaseMetadataSchema,
    );
  }

  /**
   * 获取数据库元数据（name, columns, rowCount, columnCount）
   */
  static async getDatabaseMeta(
    projectInstanceId: string,
    id: string,
    expectedRevision: number,
  ): Promise<LoadDatabaseResult> {
    const metadata = databaseMetadataSchema.parse(
      await invokeCommand<unknown>("get_database_meta", {
        projectInstanceId,
        id,
        expectedRevision,
      }),
    );
    if (metadata.id !== id)
      throw new TypeError("Database metadata identity does not match request");
    return metadata;
  }

  /**
   * 列出 SQLite 数据库中的表
   */
  static async listSqliteTables(dbPath: string): Promise<string[]> {
    return databaseSourceEntriesSchema.parse(
      await invokeCommand<unknown>("list_sqlite_tables", { dbPath }),
    );
  }

  /**
   * 列出 PostgreSQL / MySQL / MariaDB 数据库中的表
   * @param engine 引擎类型：postgres|postgresql|mysql|mariadb
   * @param connectionString 连接字符串，如 postgres://user:pass@host:5432/db 或 mysql://...
   */
  static async listSqlTables(engine: string, connectionString: string): Promise<string[]> {
    return databaseSourceEntriesSchema.parse(
      await invokeCommand<unknown>("list_sql_tables", { engine, connectionString }),
    );
  }

  /**
   * 列出 Excel 文件中的 Sheet
   */
  static async listExcelSheets(filePath: string): Promise<string[]> {
    return databaseSourceEntriesSchema.parse(
      await invokeCommand<unknown>("list_excel_sheets", { filePath }),
    );
  }

  /**
   * 删除数据库
   */
  static async deleteDatabase(
    projectInstanceId: string,
    operationId: string,
    expectedRevision: number,
    id: string,
  ): Promise<DatabaseMutationCommandResult<null>> {
    return parseDatabaseMutationResult(
      await invokeCommand<unknown>("delete_database", {
        projectInstanceId,
        operationId,
        expectedRevision,
        id,
      }),
      databaseEmptyResultSchema,
    );
  }

  /**
   * 重命名项目数据集的显示名
   */
  static async renameDatabase(
    projectInstanceId: string,
    operationId: string,
    expectedRevision: number,
    id: string,
    name: string,
  ): Promise<DatabaseMutationCommandResult<null>> {
    return parseDatabaseMutationResult(
      await invokeCommand<unknown>("rename_database", {
        projectInstanceId,
        operationId,
        expectedRevision,
        id,
        name,
      }),
      databaseEmptyResultSchema,
    );
  }

  /**
   * 获取数据库行数据（分页，含稳定 rowIds）
   */
  static async getDatabaseRows(
    projectInstanceId: string,
    id: string,
    expectedRevision: number,
    offset: number,
    limit: number,
  ): Promise<DatabaseRowsResult> {
    const payload = databaseRowsSchema.parse(
      await invokeCommand<unknown>("get_database_rows", {
        projectInstanceId,
        id,
        expectedRevision,
        offset,
        limit,
      }),
    );
    if (payload.rows.length > limit) throw new TypeError("Database page exceeds requested limit");
    return payload;
  }

  /**
   * 获取数据库所有列的分布数据（直方图/频次）
   */
  static async getColumnDistribution(
    projectInstanceId: string,
    id: string,
    expectedRevision: number,
  ): Promise<ColumnDistribution[]> {
    return databaseDistributionsSchema.parse(
      await invokeCommand<unknown>("get_column_distribution", {
        projectInstanceId,
        id,
        expectedRevision,
      }),
    );
  }

  static async getColumnValues(
    projectInstanceId: string,
    id: string,
    expectedRevision: number,
    colName: string,
  ): Promise<string[]> {
    return databaseColumnValuesSchema.parse(
      await invokeCommand<unknown>("get_column_values", {
        projectInstanceId,
        id,
        expectedRevision,
        colName,
      }),
    );
  }

  static async castColumn(
    projectInstanceId: string,
    operationId: string,
    expectedRevision: number,
    id: string,
    colName: string,
    newDtype: string,
    force = false,
  ): Promise<DatabaseMutationCommandResult<EditState>> {
    return parseDatabaseMutationResult(
      await invokeCommand<unknown>("cast_column", {
        projectInstanceId,
        operationId,
        expectedRevision,
        id,
        colName,
        newDtype,
        force,
      }),
      databaseEditStateSchema,
    );
  }

  static async setColumnSemantic(
    projectInstanceId: string,
    operationId: string,
    expectedRevision: number,
    id: string,
    colName: string,
    semantic: ColumnSemantic,
  ): Promise<DatabaseMutationCommandResult<EditState>> {
    return parseDatabaseMutationResult(
      await invokeCommand<unknown>("set_column_semantic", {
        projectInstanceId,
        operationId,
        expectedRevision,
        id,
        colName,
        semantic,
      }),
      databaseEditStateSchema,
    );
  }

  static async exportDatabase(
    projectInstanceId: string,
    id: string,
    path: string,
    format: string,
  ): Promise<void> {
    await invokeCommand("export_database", { projectInstanceId, id, path, format });
  }
}
