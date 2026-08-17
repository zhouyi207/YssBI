import { invokeCommand } from "@/services/ipc";
import type {
    ProjectSaveResultDto,
    ResourceMutationResultDto,
} from "@/shared/types/dto";


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
        return invokeCommand<ResourceMutationResultDto>("create_event", {
            projectInstanceId,
            graphName,
            operationId,
        });
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
        return invokeCommand<ResourceMutationResultDto>("create_function", {
            projectInstanceId,
            graphName,
            operationId,
        });
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
        return invokeCommand<ResourceMutationResultDto>("remove_graph", {
            projectInstanceId,
            graphPath,
            expectedRevision,
            operationId,
        });
    }


    static async unloadProjectGraph(
        graphPath: string,
        lifecycleToken: number,
        projectInstanceId: string,
    ): Promise<void> {
        await invokeCommand("unload_project_graph", { graphPath, lifecycleToken, projectInstanceId });
    }

    static async saveProjectGraph(
        projectInstanceId: string,
        graphPath: string,
        expectedRevision: number,
        operationId: string,
    ): Promise<ProjectSaveResultDto> {
        return await invokeCommand<ProjectSaveResultDto>("save_project_graph", {
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
        return invokeCommand<ResourceMutationResultDto>("duplicate_graph", {
            projectInstanceId,
            graphPath,
            expectedRevision,
            operationId,
        });
    }

    static async renameGraphResource(
        projectInstanceId: string,
        graphPath: string,
        expectedRevision: number,
        newName: string,
        lifecycleToken: number,
        operationId: string,
    ): Promise<ResourceMutationResultDto> {
        return invokeCommand<ResourceMutationResultDto>('rename_graph_resource', {
            projectInstanceId,
            graphPath,
            expectedRevision,
            newName,
            lifecycleToken,
            operationId,
        });
    }
}
