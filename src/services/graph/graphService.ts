import { invoke } from "@tauri-apps/api/core";
import { Graph } from "@/shared/types/domain";
import type {
    FunctionCallSiteDTO,
    GraphInstanceDTO,
    ProjectSaveResultDto,
    ResourceMutationResultDto,
} from "@/shared/types/dto";

import { toFrontendGraph } from "@/services/project/projectService";
import { logger } from '@/utils/appLogger';



/**
 * Graph Service - 管理 Event、Function 资源生命周期与函数引用查询
 *
 * 创建时即分配 `events/…` / `functions/…` 路径并写入磁盘；正文在打开 tab 时从文件加载。
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

    static async unloadProjectGraph(
        graphPath: string,
        lifecycleToken: number,
        projectInstanceId: string,
    ): Promise<void> {
        await invoke("unload_project_graph", { graphPath, lifecycleToken, projectInstanceId });
    }

    static async saveProjectGraph(
        projectInstanceId: string,
        graphPath: string,
        expectedRevision: number,
        operationId: string,
    ): Promise<ProjectSaveResultDto> {
        return await invoke<ProjectSaveResultDto>("save_project_graph", {
            projectInstanceId,
            graphPath,
            expectedRevision,
            operationId,
        });
    }

    static async duplicateGraph(graphPath: string): Promise<string> {
        const newPath = await invoke<string>("duplicate_graph", { graphPath });
        logger.graph.info(`Graph '${graphPath}' duplicated to '${newPath}'`, 'GraphService');
        return newPath;
    }

    static async renameGraphResource(
        projectInstanceId: string,
        graphPath: string,
        newName: string,
    ): Promise<ResourceMutationResultDto> {
        return invoke<ResourceMutationResultDto>('rename_graph_resource', {
            projectInstanceId,
            graphPath,
            newName,
        });
    }
}
