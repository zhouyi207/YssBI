import { invoke } from "@tauri-apps/api/core";
import { Graph } from "@/shared/types/domain";
import type { FunctionSignaturePatch } from "@/shared/types";
import type { GraphInstanceDTO, FunctionCallSiteDTO } from "@/shared/types/dto";
import { markResourceLoaded } from "@/features/core/resource";
import type { BackendProjectResourceMeta } from "@/features/core/resource";
import { inferGraphResourceKind } from "@/shared/types/domain/graphResourcePath";
import { toFrontendGraph } from "@/services/project/projectService";
import { logger } from '@/utils/appLogger';

/**
 * Graph Service - 管理 Event、Function 的创建、删除、更新和查询
 *
 * 创建时即分配 `events/…` / `functions/…` 路径；正文由 EventCreated/FunctionCreated 事件注入并保持 loaded。
 */
export class GraphService {
    /**
     * 创建 Event
     * @param graphName - Event 的名称
     * @returns graph path（`events/…`）
     */
    static async createEvent(graphName: string): Promise<string> {
        try {
            const graphPath = await invoke<string>("create_event", { graphName });
            logger.graph.info(`Event '${graphName}' created with path: ${graphPath}`, 'GraphService');
            return graphPath;
        } catch (error) {
            logger.graph.error(`Error creating event: ${error instanceof Error ? error.message : String(error)}`, 'GraphService');
            throw error;
        }
    }

    /**
     * 创建 Function
     * @param graphName - Function 的名称
     * @returns graph path（`functions/…`）
     */
    static async createFunction(graphName: string): Promise<string> {
        try {
            const graphPath = await invoke<string>("create_function", { graphName });
            logger.graph.info(`Function '${graphName}' created with path: ${graphPath}`, 'GraphService');
            return graphPath;
        } catch (error) {
            logger.graph.error(`Error creating function: ${error instanceof Error ? error.message : String(error)}`, 'GraphService');
            throw error;
        }
    }

    /**
     * 删除 Graph (Event/Function)
     * @param graphPath - Graph 路径
     */
    static async removeGraph(graphPath: string): Promise<void> {
        try {
            await invoke("remove_graph", { graphPath });
            logger.graph.info(`Graph '${graphPath}' removed successfully`, 'GraphService');
        } catch (error) {
            logger.graph.error(`Error removing graph: ${error instanceof Error ? error.message : String(error)}`, 'GraphService');
            throw error;
        }
    }

    static async updateFunctionSignature(
        functionPath: string,
        patch: FunctionSignaturePatch,
    ): Promise<{ graph: Graph; callerGraphs: Graph[] }> {
        try {
            const result = await invoke<{
                graph: GraphInstanceDTO;
                callerGraphs: GraphInstanceDTO[];
            }>(
                "update_function_signature",
                {
                    functionPath,
                    inputs: patch.inputs,
                    outputs: patch.outputs,
                },
            );
            logger.graph.info(`Function '${functionPath}' signature updated successfully`, 'GraphService');
            return {
                graph: toFrontendGraph(result.graph),
                callerGraphs: (result.callerGraphs ?? []).map(toFrontendGraph),
            };
        } catch (error) {
            logger.graph.error(`Error updating function signature: ${error instanceof Error ? error.message : String(error)}`, 'GraphService');
            throw error;
        }
    }

    /**
     * 获取 Graph 详情
     * @param graphPath - Graph 路径
     * @returns Graph 对象
     */
    static async getGraph(graphPath: string): Promise<Graph> {
        try {
            const graph = await invoke<GraphInstanceDTO>("get_graph", { graphPath });
            logger.graph.info(`Graph '${graphPath}' retrieved successfully`, 'GraphService');
            return toFrontendGraph(graph);
        } catch (error) {
            logger.graph.error(`Error getting graph: ${error instanceof Error ? error.message : String(error)}`, 'GraphService');
            throw error;
        }
    }

    static async resolveGraphDynamicPins(graphPath: string): Promise<Graph> {
        try {
            const graph = await invoke<GraphInstanceDTO>("resolve_graph_dynamic_pins", { graphPath });
            logger.graph.info(`Graph '${graphPath}' dynamic pins materialized`, 'GraphService');
            return toFrontendGraph(graph);
        } catch (error) {
            logger.graph.error(`Error resolving graph dynamic pins: ${error instanceof Error ? error.message : String(error)}`, 'GraphService');
            throw error;
        }
    }

    static async getFunctionCallSites(functionPath: string): Promise<FunctionCallSiteDTO[]> {
        return invoke<FunctionCallSiteDTO[]>("get_function_call_sites", { functionPath });
    }

    static async updateCallFunctionTarget(
        graphPath: string,
        nodeId: string,
        functionPath: string,
    ): Promise<void> {
        try {
            await invoke("update_call_function_target", { graphPath, nodeId, functionPath });
            logger.graph.info(
                `Call node '${nodeId}' rebound to function '${functionPath}'`,
                'GraphService',
            );
        } catch (error) {
            logger.graph.error(
                `Error rebinding Call Function target: ${error instanceof Error ? error.message : String(error)}`,
                'GraphService',
            );
            throw error;
        }
    }

    static async purgeFunctionCallSites(functionPath: string): Promise<Graph[]> {
        const graphs = await invoke<GraphInstanceDTO[]>("purge_function_call_sites", { functionPath });
        return graphs.map(toFrontendGraph);
    }

    static async unloadProjectGraph(graphPath: string): Promise<void> {
        await invoke("unload_project_graph", { graphPath });
        const kind = inferGraphResourceKind(graphPath);
        if (kind) {
            markResourceLoaded({ id: graphPath, kind }, false);
        }
    }

    static async saveProjectGraph(graphPath: string): Promise<string> {
        const result = await invoke<{ path: string }>("save_project_graph", { graphPath });
        return result.path;
    }

    static async duplicateGraph(graphPath: string): Promise<Graph> {
        const graph = await invoke<GraphInstanceDTO>("duplicate_graph", { graphPath });
        return toFrontendGraph(graph);
    }

    static async renameGraphResource(
        graphPath: string,
        newName: string,
    ): Promise<BackendProjectResourceMeta> {
        return invoke('rename_graph_resource', { graphPath, newName });
    }
}
