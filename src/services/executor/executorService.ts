import { invoke } from "@tauri-apps/api/core";

/**
 * Executor Service
 * 执行服务 - 封装图执行相关的后端调用
 */
export class ExecutorService {
    // ==================== 执行 ====================

    /**
     * 执行当前项目（从状态管理器获取数据）
     */
    static async execute(): Promise<string[]> {
        return await invoke("execute_graph");
    }

    /**
     * 执行指定的项目数据
     */
    static async executeProject(
        Variables: Record<string, any>,
        events: Record<string, any>,
        functions: Record<string, any>,
        macros: Record<string, any>,
        dataframes: Record<string, any> = {}
    ): Promise<string> {
        const backendData = {
            Variables,
            events,
            functions,
            macros,
            dataframes,
            metadata: {
                exportTime: new Date().toISOString(),
                appVersion: "0.1.0"
            },
        };
        const res: string[] = await invoke("execute_project", { data: backendData });
        return res.join("\n");
    }
}