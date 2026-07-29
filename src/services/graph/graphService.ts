import { invoke } from "@tauri-apps/api/core";
import type {
    ProjectSaveResultDto,
    ResourceMutationResultDto,
} from "@/shared/types/dto";

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
    static async createEvent(
        projectInstanceId: string,
        graphName: string,
        operationId: string,
    ): Promise<ResourceMutationResultDto> {
        try {
            const result = await invoke<ResourceMutationResultDto>("create_event", {
                projectInstanceId,
                graphName,
                operationId,
            });
            logger.graph.info(`Event '${graphName}' created`, 'GraphService');
            return result;
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
    static async createFunction(
        projectInstanceId: string,
        graphName: string,
        operationId: string,
    ): Promise<ResourceMutationResultDto> {
        try {
            const result = await invoke<ResourceMutationResultDto>("create_function", {
                projectInstanceId,
                graphName,
                operationId,
            });
            logger.graph.info(`Function '${graphName}' created`, 'GraphService');
            return result;
        } catch (error) {
            logger.graph.error(`Error creating function: ${error instanceof Error ? error.message : String(error)}`, 'GraphService');
            throw error;
        }
    }

    /**
     * 删除 Graph (Event/Function)
     * @param graphPath - Graph 路径
     */
    static async removeGraph(
        projectInstanceId: string,
        graphPath: string,
        expectedRevision: number,
        operationId: string,
    ): Promise<ResourceMutationResultDto> {
        try {
            const result = await invoke<ResourceMutationResultDto>("remove_graph", {
                projectInstanceId,
                graphPath,
                expectedRevision,
                operationId,
            });
            logger.graph.info(`Graph '${graphPath}' removed successfully`, 'GraphService');
            return result;
        } catch (error) {
            logger.graph.error(`Error removing graph: ${error instanceof Error ? error.message : String(error)}`, 'GraphService');
            throw error;
        }
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

    static async duplicateGraph(
        projectInstanceId: string,
        graphPath: string,
        expectedRevision: number,
        operationId: string,
    ): Promise<ResourceMutationResultDto> {
        const result = await invoke<ResourceMutationResultDto>("duplicate_graph", {
            projectInstanceId,
            graphPath,
            expectedRevision,
            operationId,
        });
        logger.graph.info(`Graph '${graphPath}' duplicated`, 'GraphService');
        return result;
    }

    static async renameGraphResource(
        projectInstanceId: string,
        graphPath: string,
        expectedRevision: number,
        newName: string,
        lifecycleToken: number,
        operationId: string,
    ): Promise<ResourceMutationResultDto> {
        return invoke<ResourceMutationResultDto>('rename_graph_resource', {
            projectInstanceId,
            graphPath,
            expectedRevision,
            newName,
            lifecycleToken,
            operationId,
        });
    }
}
