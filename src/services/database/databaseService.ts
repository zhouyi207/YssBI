import { invoke } from "@tauri-apps/api/core";

/**
 * Database Service
 * 数据库服务 - 封装数据帧相关的后端调用
 */
export class DatabaseService {
    /**
     * 从 CSV 导入数据
     */
    static async importCSV(path: string): Promise<any> {
        return await invoke("import_csv", { path });
    }

    /**
     * 删除数据帧
     */
    static async deleteDataFrame(id: string): Promise<void> {
        await invoke("delete_dataframe", { id });
    }

    /**
     * 创建数据帧（手动）
     */
    static async createDataFrame(id: string, data: any): Promise<any> {
        return await invoke("create_dataframe", { id, data });
    }

    /**
     * 获取数据帧行数据
     */
    static async getDataFrameRows(id: string, offset: number, limit: number): Promise<any[][]> {
        return await invoke("get_dataframe_rows", { id, offset, limit });
    }
}