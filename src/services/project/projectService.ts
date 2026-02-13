import { save, open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { Graph, ProjectData, CanvasState, Pin } from "@/shared/types/editor";



/**
 * 将后端 SubGraphData 转换为前端格式
 * 目前前后端使用相同的 JSON 字段名，直接返回
 */
function toFrontendSubGraphData(data: any): Graph {
    return data as Graph;
}

/**
 * 将后端 HashMap 转换为前端 Record
 */
function convertSubGraphMap(map: Record<string, any>): Record<string, Graph> {
    const result: Record<string, Graph> = {};
    for (const [id, data] of Object.entries(map)) {
        result[id] = toFrontendSubGraphData(data);
    }
    return result;
}

// ==================== 项目状态管理 API ====================

export class ProjectService {
    // ==================== 项目级操作 ====================

    /**
     * 获取当前项目状态
     */
    static async getProjectState(): Promise<ProjectData> {
        console.log('[ProjectService.getProjectState] Invoking get_project_data...');
        const data: any = await invoke("get_project_data");
        console.log('[ProjectService.getProjectState] Raw backend data:', data);
        // 后端使用 serde rename，JSON 字段名是 camelCase (globalVariables)
        const result = {
            globalVariables: data.globalVariables || {},
            events: convertSubGraphMap(data.events || {}),
            functions: convertSubGraphMap(data.functions || {}),
            macros: convertSubGraphMap(data.macros || {}),
            dataframes: data.dataframes || {},
            metadata: data.metadata || { exportTime: "", appVersion: "" },
        };
        console.log('[ProjectService.getProjectState] Converted data:', result);
        return result;
    }

    /**
     * 获取当前项目路径
     */
    static async getProjectPath(): Promise<string | null> {
        return await invoke("get_project_path");
    }

    /**
     * 新建项目（清空当前状态）
     */
    static async newProject(): Promise<void> {
        await invoke("new_project");
    }

    /**
     * 从文件加载项目到状态管理器
     */
    static async loadProjectToState(path?: string): Promise<{ project: ProjectData; path: string | null } | null> {
        try {
            let filePath = path;
            if (!filePath) {
                // 弹出文件选择对话框
                const selected = await open({
                    multiple: false,
                    filters: [{ name: "YssBI Project", extensions: ["json"] }]
                });
                if (!selected || Array.isArray(selected)) return null;
                filePath = selected as string;
            }

            const data: any = await invoke("load_project_to_state", { path: filePath });
            return {
                project: {
                    globalVariables: data.globalVariables || {},
                    events: convertSubGraphMap(data.events || {}),
                    functions: convertSubGraphMap(data.functions || {}),
                    macros: convertSubGraphMap(data.macros || {}),
                    dataframes: data.dataframes || {},
                    metadata: data.metadata || { exportTime: "", appVersion: "" },
                },
                path: filePath,
            };
        } catch (e) {
            console.error("Failed to load project:", e);
            throw e;
        }
    }

    /**
     * 保存当前项目状态到文件
     */
    static async saveProjectFromState(path?: string): Promise<string | null> {
        try {
            let filePath: string | undefined = path;
            if (!filePath) {
                const selected = await save({
                    filters: [{ name: "YssBI Project", extensions: ["json"] }]
                });
                if (!selected) return null;
                filePath = selected;
            }
            await invoke("save_project_from_state", { path: filePath });
            return filePath;
        } catch (e) {
            console.error("Failed to save project:", e);
            throw e;
        }
    }

    /**
     * 设置完整的项目数据（用于批量同步）
     * 注意：后端使用 serde rename，JSON 字段名使用 camelCase
     */
    static async setProjectData(data: ProjectData, path?: string, emitEvent: boolean = false): Promise<void> {
        // 直接发送，字段名已经匹配（前端和后端都使用 camelCase 的 JSON 字段名）
        const backendData = {
            globalVariables: data.globalVariables,
            events: data.events,
            functions: data.functions,
            macros: data.macros,
            dataframes: data.dataframes,
            metadata: data.metadata,
        };
        console.log('[ProjectService.setProjectData] Sending to backend:', {
            eventsCount: Object.keys(backendData.events).length,
            functionsCount: Object.keys(backendData.functions).length,
            macrosCount: Object.keys(backendData.macros).length,
            globalVariablesCount: Object.keys(backendData.globalVariables).length,
            dataframesCount: Object.keys(backendData.dataframes || {}).length,
            emitEvent,
        });
        await invoke("set_project_data", { data: backendData, path: path || null, emitEvent });
        console.log('[ProjectService.setProjectData] Successfully sent to backend');
    }



    static async updateCanvas(subgraphId: string, canvas: CanvasState): Promise<void> {
        await invoke("update_canvas", { subgraphId, canvas });
    }

    static async updateSubgraphIo(
        subgraphId: string,
        inputs?: Pin[],
        outputs?: Pin[]
    ): Promise<Graph> {
        const result: any = await invoke("update_subgraph_io", {
            subgraphId,
            inputs: inputs || null,
            outputs: outputs || null,
        });
        return toFrontendSubGraphData(result);
    }

    static async renameSubgraph(subgraphId: string, newName: string): Promise<Graph> {
        const result: any = await invoke("rename_subgraph", { subgraphId, newName });
        return toFrontendSubGraphData(result);
    }

}
