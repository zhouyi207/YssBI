import { invoke } from "@tauri-apps/api/core";
import { Graph } from "@/shared/types/domain";
import { logger } from '@/utils/appLogger';

/**
 * Graph Service - 管理 Event、Function、Macro 的创建、删除、更新和查询
 * 
 * 注意：创建方法只需要传递 graph_name，后端会自动创建完整的 Graph 结构
 */
export class GraphService {
    /**
     * 创建 Event
     * @param graphName - Event 的名称
     * @returns 后端生成的 Graph ID
     */
    static async createEvent(graphName: string): Promise<string> {
        try {
            const id = await invoke<string>("create_event", { graphName });
            logger.graph.info(`Event '${graphName}' created with ID: ${id}`, 'GraphService');
            return id;
        } catch (error) {
            logger.graph.error(`Error creating event: ${error instanceof Error ? error.message : String(error)}`, 'GraphService');
            throw error;
        }
    }

    /**
     * 创建 Function
     * @param graphName - Function 的名称
     * @returns 后端生成的 Graph ID
     */
    static async createFunction(graphName: string): Promise<string> {
        try {
            const id = await invoke<string>("create_function", { graphName });
            logger.graph.info(`Function '${graphName}' created with ID: ${id}`, 'GraphService');
            return id;
        } catch (error) {
            logger.graph.error(`Error creating function: ${error instanceof Error ? error.message : String(error)}`, 'GraphService');
            throw error;
        }
    }

    /**
     * 创建 Macro
     * @param graphName - Macro 的名称
     * @returns 后端生成的 Graph ID
     */
    static async createMacro(graphName: string): Promise<string> {
        try {
            const id = await invoke<string>("create_macro", { graphName });
            logger.graph.info(`Macro '${graphName}' created with ID: ${id}`, 'GraphService');
            return id;
        } catch (error) {
            logger.graph.error(`Error creating macro: ${error instanceof Error ? error.message : String(error)}`, 'GraphService');
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
            logger.graph.info(`Graph '${graphId}' removed successfully`, 'GraphService');
        } catch (error) {
            logger.graph.error(`Error removing graph: ${error instanceof Error ? error.message : String(error)}`, 'GraphService');
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
            logger.graph.info(`Event '${id}' updated successfully`, 'GraphService');
        } catch (error) {
            logger.graph.error(`Error updating event: ${error instanceof Error ? error.message : String(error)}`, 'GraphService');
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
            logger.graph.info(`Function '${id}' updated successfully`, 'GraphService');
        } catch (error) {
            logger.graph.error(`Error updating function: ${error instanceof Error ? error.message : String(error)}`, 'GraphService');
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
            logger.graph.info(`Macro '${id}' updated successfully`, 'GraphService');
        } catch (error) {
            logger.graph.error(`Error updating macro: ${error instanceof Error ? error.message : String(error)}`, 'GraphService');
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
            logger.graph.info(`Graph '${graphId}' retrieved successfully`, 'GraphService');
            return graph;
        } catch (error) {
            logger.graph.error(`Error getting graph: ${error instanceof Error ? error.message : String(error)}`, 'GraphService');
            throw error;
        }
    }
}
