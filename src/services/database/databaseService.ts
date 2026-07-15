import { invoke } from "@tauri-apps/api/core";
import type {
    ColumnDistribution,
    ColumnStats,
    DatasetOverview,
    EditState,
} from "@/shared/types/domain/dataframe";
import type { DatabaseRow, LoadDatabaseEngineSpec, LoadDatabaseResult } from "@/shared/types/dto/database";

export type { LoadDatabaseEngineSpec } from "@/shared/types/dto/database";

/** 分页行数据（含稳定 rowIds） */
export interface DatabaseRowsResult {
    rows: DatabaseRow[];
    rowIds: number[];
}

/**
 * Database Service
 * 数据库服务 - 封装 load_database、delete_database、get_database_rows
 */
export class DatabaseService {
    /**
     * 加载数据库（CSV、Parquet 等）
     */
    static async loadDatabase(engine: LoadDatabaseEngineSpec): Promise<LoadDatabaseResult> {
        return await invoke("load_database", { engine });
    }

    /**
     * 获取数据库元数据（name, columns, rowCount, columnCount）
     */
    static async getDatabaseMeta(id: string): Promise<LoadDatabaseResult> {
        return await invoke("get_database_meta", { id });
    }

    /**
     * 列出 SQLite 数据库中的表
     */
    static async listSqliteTables(dbPath: string): Promise<string[]> {
        return await invoke("list_sqlite_tables", { dbPath });
    }

    /**
     * 列出 PostgreSQL / MySQL / MariaDB 数据库中的表
     * @param engine 引擎类型：postgres|postgresql|mysql|mariadb
     * @param connectionString 连接字符串，如 postgres://user:pass@host:5432/db 或 mysql://...
     */
    static async listSqlTables(engine: string, connectionString: string): Promise<string[]> {
        return await invoke("list_sql_tables", { engine, connectionString });
    }

    /**
     * 列出 Excel 文件中的 Sheet
     */
    static async listExcelSheets(filePath: string): Promise<string[]> {
        return await invoke("list_excel_sheets", { filePath });
    }

    /**
     * 删除数据库
     */
    static async deleteDatabase(id: string): Promise<void> {
        await invoke("delete_database", { id });
    }

    /**
     * 重命名数据库（显示名，写入 DuckDB meta）
     */
    static async renameDatabase(id: string, name: string): Promise<void> {
        await invoke("rename_database", { id, name });
    }

    /**
     * 获取数据库行数据（分页，含稳定 rowIds）
     */
    static async getDatabaseRows(id: string, offset: number, limit: number): Promise<DatabaseRowsResult> {
        const payload = await invoke<{ rows?: DatabaseRow[]; rowIds?: number[] } | DatabaseRow[]>(
            "get_database_rows",
            { id, offset, limit },
        );
        if (Array.isArray(payload)) {
            return { rows: payload, rowIds: [] };
        }
        return {
            rows: payload.rows ?? [],
            rowIds: payload.rowIds ?? [],
        };
    }

    /**
     * 获取数据库所有列的统计信息
     */
    static async getColumnStats(id: string): Promise<ColumnStats[]> {
        return await invoke("get_column_stats", { id });
    }

    /**
     * 获取数据库所有列的分布数据（直方图/频次）
     */
    static async getColumnDistribution(id: string): Promise<ColumnDistribution[]> {
        return await invoke("get_column_distribution", { id });
    }

    static async getDatasetOverview(id: string): Promise<DatasetOverview> {
        return await invoke("get_dataset_overview", { id });
    }

    static async editCell(
        id: string,
        row: number,
        colName: string,
        value: unknown,
        rowId?: number | null,
    ): Promise<EditState> {
        return await invoke("edit_cell", { id, row, colName, value, rowId: rowId ?? null });
    }

    static async addRow(id: string, index?: number): Promise<EditState> {
        return await invoke("add_row", { id, index: index ?? null });
    }

    static async deleteRows(id: string, indices: number[], rowIds?: number[]): Promise<EditState> {
        return await invoke("delete_rows", {
            id,
            indices,
            rowIds: rowIds && rowIds.length > 0 ? rowIds : null,
        });
    }

    static async addColumn(id: string, name: string, dtype: string): Promise<EditState> {
        return await invoke("add_column", { id, name, dtype });
    }

    static async deleteColumn(id: string, name: string): Promise<EditState> {
        return await invoke("delete_column", { id, name });
    }

    static async castColumn(id: string, colName: string, newDtype: string, force = false): Promise<EditState> {
        return await invoke("cast_column", { id, colName, newDtype, force });
    }

    static async renameColumn(id: string, oldName: string, newName: string): Promise<EditState> {
        return await invoke("rename_column", { id, oldName, newName });
    }

    static async undoEdit(id: string): Promise<EditState> {
        return await invoke("undo_edit", { id });
    }

    static async redoEdit(id: string): Promise<EditState> {
        return await invoke("redo_edit", { id });
    }

    static async saveDatabaseChanges(id: string): Promise<EditState> {
        return await invoke("save_database_changes", { id });
    }

    static async exportDatabase(id: string, path: string, format: string): Promise<void> {
        await invoke("export_database", { id, path, format });
    }

    static async getEditState(id: string): Promise<EditState> {
        return await invoke("get_edit_state", { id });
    }
}
