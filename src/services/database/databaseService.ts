import { invoke } from "@tauri-apps/api/core";
import { ColumnStats } from "@/features/core/dataStore/columnStatsStore";
import { ColumnDistribution } from "@/features/core/dataStore/columnDistributionStore";
import { DatasetOverview } from "@/features/core/dataStore/datasetOverviewStore";

/** CSV 引擎配置（与后端 DatabaseEngineDTO::Csv 对应） */
export interface CsvEngineSpec {
    path: string;
    delimiter?: string;
    hasHeader?: boolean;
    inferSchemaLength?: number;
}

/** Parquet 引擎配置 */
export interface ParquetEngineSpec {
    path: string;
    columns?: string[];
}

/** 加载数据库的引擎配置（与后端 DatabaseEngineDTO 对应） */
export type LoadDatabaseEngineSpec =
    | { csv: CsvEngineSpec }
    | { parquet: ParquetEngineSpec };

/** 加载结果（与后端 LoadDatabaseResult 对应） */
export interface LoadDatabaseResult {
    id: string;
    name: string;
    rowCount: number;
    columnCount: number;
    columns: Array<{ name: string; type: string }>;
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
     * 删除数据库
     */
    static async deleteDatabase(id: string): Promise<void> {
        await invoke("delete_database", { id });
    }

    /**
     * 获取数据库行数据（分页）
     */
    static async getDatabaseRows(id: string, offset: number, limit: number): Promise<any[][]> {
        return await invoke("get_database_rows", { id, offset, limit });
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
}
