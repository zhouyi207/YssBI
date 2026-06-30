import { invoke } from "@tauri-apps/api/core";
import { Graph } from "@/shared/types/domain";
import type { FunctionSignaturePatch } from "@/shared/types";
import type { GraphInstanceDTO } from "@/shared/types/dto";
import { toFrontendGraph } from "@/services/project/projectService";
import { logger } from '@/utils/appLogger';

/**
 * Graph Service - 管理 Event、Function 的创建、删除、更新和查询
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
            await this.unloadProjectGraph(id);
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
            await this.unloadProjectGraph(id);
            return id;
        } catch (error) {
            logger.graph.error(`Error creating function: ${error instanceof Error ? error.message : String(error)}`, 'GraphService');
            throw error;
        }
    }

    /**
     * 删除 Graph (Event/Function)
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

    static async updateFunctionSignature(functionId: string, patch: FunctionSignaturePatch): Promise<Graph> {
        try {
            const graph = await invoke<GraphInstanceDTO>("update_function_signature", {
                functionId,
                inputs: patch.inputs,
                outputs: patch.outputs,
            });
            logger.graph.info(`Function '${functionId}' signature updated successfully`, 'GraphService');
            return toFrontendGraph(graph);
        } catch (error) {
            logger.graph.error(`Error updating function signature: ${error instanceof Error ? error.message : String(error)}`, 'GraphService');
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
            const graph = await invoke<GraphInstanceDTO>("get_graph", { graphId });
            logger.graph.info(`Graph '${graphId}' retrieved successfully`, 'GraphService');
            return toFrontendGraph(graph);
        } catch (error) {
            logger.graph.error(`Error getting graph: ${error instanceof Error ? error.message : String(error)}`, 'GraphService');
            throw error;
        }
    }

    static async resolveGraphDynamicPins(graphId: string): Promise<Graph> {
        try {
            const graph = await invoke<GraphInstanceDTO>("resolve_graph_dynamic_pins", { graphId });
            logger.graph.info(`Graph '${graphId}' dynamic pins materialized`, 'GraphService');
            return toFrontendGraph(graph);
        } catch (error) {
            logger.graph.error(`Error resolving graph dynamic pins: ${error instanceof Error ? error.message : String(error)}`, 'GraphService');
            throw error;
        }
    }

    static async unloadProjectGraph(graphId: string): Promise<void> {
        await invoke("unload_project_graph", { graphId });
    }

    static async saveProjectGraph(graphId: string): Promise<void> {
        await invoke("save_project_graph", { graphId });
    }

    static async duplicateGraph(graphId: string): Promise<Graph> {
        const graph = await invoke<GraphInstanceDTO>("duplicate_graph", { graphId });
        await this.unloadProjectGraph(graph.id);
        return toFrontendGraph(graph);
    }
}
