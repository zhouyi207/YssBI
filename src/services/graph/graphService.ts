import { invoke } from "@tauri-apps/api/core";
import { Graph } from "@/shared/types/domain";

/**
 * Graph Service - 管理 Event、Function、Macro 的创建、删除、更新和查询
 * 
 * 注意：创建方法只需要传递 graph_name，后端会自动创建完整的 Graph 结构
 */
export class GraphService {
    /**
     * 创建 Event
     * @param graphName - Event 的名称
     */
    static async createEvent(graphName: string): Promise<void> {
        try {
            await invoke("create_event", { graphName });
            console.log(`[GraphService.createEvent] Event '${graphName}' created successfully`);
        } catch (error) {
            console.error("[GraphService.createEvent] Error creating event:", error);
            throw error;
        }
    }

    /**
     * 创建 Function
     * @param graphName - Function 的名称
     */
    static async createFunction(graphName: string): Promise<void> {
        try {
            await invoke("create_function", { graphName });
            console.log(`[GraphService.createFunction] Function '${graphName}' created successfully`);
        } catch (error) {
            console.error("[GraphService.createFunction] Error creating function:", error);
            throw error;
        }
    }

    /**
     * 创建 Macro
     * @param graphName - Macro 的名称
     */
    static async createMacro(graphName: string): Promise<void> {
        try {
            await invoke("create_macro", { graphName });
            console.log(`[GraphService.createMacro] Macro '${graphName}' created successfully`);
        } catch (error) {
            console.error("[GraphService.createMacro] Error creating macro:", error);
            throw error;
        }
    }

    /**
     * 删除 Graph (Event/Function/Macro)
     * @param graphId - Graph 的 ID
     */
    static async removeGraph(graphId: string): Promise<void> {
        try {
            await invoke("remove_graph", { graphId });
            console.log(`[GraphService.removeGraph] Graph '${graphId}' removed successfully`);
        } catch (error) {
            console.error("[GraphService.removeGraph] Error removing graph:", error);
            throw error;
        }
    }

    /**
     * 更新 Event
     * @param id - Event 的 ID
     * @param event - 更新的 Event 数据
     */
    static async updateEvent(id: string, event: Graph): Promise<void> {
        try {
            await invoke("update_event", { id, event });
            console.log(`[GraphService.updateEvent] Event '${id}' updated successfully`);
        } catch (error) {
            console.error("[GraphService.updateEvent] Error updating event:", error);
            throw error;
        }
    }

    /**
     * 更新 Function
     * @param id - Function 的 ID
     * @param functionData - 更新的 Function 数据
     */
    static async updateFunction(id: string, functionData: Graph): Promise<void> {
        try {
            await invoke("update_function", { id, function: functionData });
            console.log(`[GraphService.updateFunction] Function '${id}' updated successfully`);
        } catch (error) {
            console.error("[GraphService.updateFunction] Error updating function:", error);
            throw error;
        }
    }

    /**
     * 更新 Macro
     * @param id - Macro 的 ID
     * @param macroData - 更新的 Macro 数据
     */
    static async updateMacro(id: string, macroData: Graph): Promise<void> {
        try {
            await invoke("update_macro", { id, macroData });
            console.log(`[GraphService.updateMacro] Macro '${id}' updated successfully`);
        } catch (error) {
            console.error("[GraphService.updateMacro] Error updating macro:", error);
            throw error;
        }
    }

    /**
     * 获取 Graph 详情
     * @param graphId - Graph 的 ID
     * @returns Graph 对象
     */
    static async getGraph(graphId: string): Promise<Graph> {
        try {
            const graph = await invoke<Graph>("get_graph", { graphId });
            console.log(`[GraphService.getGraph] Graph '${graphId}' retrieved successfully`);
            return graph;
        } catch (error) {
            console.error("[GraphService.getGraph] Error getting graph:", error);
            throw error;
        }
    }
}
