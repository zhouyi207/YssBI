import { save, open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { Graph, ProjectData, GraphPosition, Pin } from "@/shared/types/editor";

type CanvasState = GraphPosition;

/**
 * 将后端 Graph 数据转换为前端格式
 */
function toFrontendGraph(data: any): Graph {
    // 后端返回的结构需要转换
    // connections 是一个对象：{ connections: {}, reverse_connections: {}, ... }
    // 需要将 connections 对象转换为数组格式
    const backendConnections = data.data_state?.connections?.connections || {};
    const connectionsArray: any[] = [];
    
    // 将对象转换为数组
    // 假设格式是 { "pin1->pin2": { from_pin: "pin1", to_pin: "pin2" }, ... }
    // 或者是 { "key": [from_pin, to_pin], ... }
    for (const [key, value] of Object.entries(backendConnections)) {
        if (Array.isArray(value) && value.length === 2) {
            // 格式：{ "key": [from_pin, to_pin] }
            connectionsArray.push({
                from_pin: value[0],
                to_pin: value[1]
            });
        } else if (typeof value === 'object' && value !== null) {
            // 格式：{ "key": { from_pin: "...", to_pin: "..." } }
            connectionsArray.push(value);
        }
    }
    
    console.log('[toFrontendGraph] Converted connections:', {
        backend: backendConnections,
        frontend: connectionsArray
    });
    
    return {
        id: data.id,
        name: data.name,
        type: data.kind.toLowerCase() as "event" | "function" | "macro", // 后端是 "Event"/"Function"/"Macro"
        nodes: [], // TODO: 从 data_state.nodes 转换
        pins: [], // TODO: 从 data_state.pins 转换
        connections: { connections: connectionsArray },
        canvas: data.position || { x: 0, y: 0, scale: 1 }
    };
}

/**
 * 将后端 Graph Map 转换为前端 Record
 */
function convertGraphMap(map: Record<string, any>): Record<string, Graph> {
    const result: Record<string, Graph> = {};
    for (const [id, data] of Object.entries(map)) {
        result[id] = toFrontendGraph(data);
    }
    return result;
}

// ==================== 项目状态管理 API ====================

export class ProjectService {
    // ==================== 项目级操作 ====================

    /**
     * 获取当前项目状态 - 使用新的 ProjectData 结构
     */
    static async getProjectState(): Promise<ProjectData> {
        console.log('[ProjectService.getProjectState] Invoking get_project_data...');
        const data: any = await invoke("get_project_data");
        console.log('[ProjectService.getProjectState] Raw backend data:', JSON.stringify(data));
        
        // 新格式：直接使用 variables, graphs, databases
        const result: ProjectData = {
            variables: data.variables || {},
            graphs: convertGraphMap(data.graphs || {}),
            databases: data.databases || {},
            metadata: data.metadata || { exportTime: "", appVersion: "" },
        };
        
        console.log('[ProjectService.getProjectState] Converted data:', {
            variablesCount: Object.keys(result.variables).length,
            graphsCount: Object.keys(result.graphs).length,
            databasesCount: Object.keys(result.databases).length,
            graphs: result.graphs
        });
        
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
     * 从文件加载项目到状态管理器 - 使用新的 ProjectData 结构
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
                    variables: data.variables || {},
                    graphs: convertGraphMap(data.graphs || {}),
                    databases: data.databases || {},
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
     * 设置完整的项目数据（用于批量同步）- 使用新的 ProjectData 结构
     */
    static async setProjectData(data: ProjectData, path?: string, emitEvent: boolean = false): Promise<void> {
        // 直接发送新格式数据
        const backendData = {
            variables: data.variables,
            graphs: data.graphs,
            databases: data.databases,
            metadata: data.metadata,
        };
        console.log('[ProjectService.setProjectData] Sending to backend:', {
            variablesCount: Object.keys(backendData.variables).length,
            graphsCount: Object.keys(backendData.graphs).length,
            databasesCount: Object.keys(backendData.databases || {}).length,
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
        return toFrontendGraph(result);
    }

    static async renameSubgraph(subgraphId: string, newName: string): Promise<Graph> {
        const result: any = await invoke("rename_subgraph", { subgraphId, newName });
        return toFrontendGraph(result);
    }

}
