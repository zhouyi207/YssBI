import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { invoke, Channel } from "@tauri-apps/api/core";
import type { ExecuteGraphResultDto, RunEvent } from "@/shared/types/dto/runEvent";
import type { Graph } from "@/shared/types/domain";
import type { GraphInstanceDTO } from "@/shared/types/dto";
import type { HistoryStatusDto } from "@/shared/types/dto/editorMutation";
import type { CleanupInvalidProjectsResult, LifecycleMutationResultDto, ProjectPathValidation, ProjectRecordRow, ScanProjectsResult } from "@/shared/types/dto/project";
import {
  graphDataToDomainGraph,
  graphInstanceDtoToGraphData,
} from "@/shared/types/dto/graphModel";
import { logger } from '@/utils/appLogger';
import { formatErrorMessage } from "@/shared/utils/formatErrorMessage";
import { trackChannel, untrackChannel } from "@/services/devHmrIpc";
import { bindExecutionEventChannel } from "./executionChannelDrain";

export type ProjectScanProgressEvent =
    | { kind: "scanning" }
    | { kind: "discovered"; count: number }
    | { kind: "registering"; current: number; total: number };

export type ProjectCleanupProgressEvent =
    | { kind: "checking"; current: number; total: number }
    | { kind: "removing"; removed: number; total: number };

export const PICKER_TASK_CANCELLED = "PICKER_TASK_CANCELLED";

export function isPickerTaskCancelledError(error: unknown): boolean {
    return formatErrorMessage(error, "") === PICKER_TASK_CANCELLED;
}

export function isExecutionCancelledError(error: unknown): boolean {
    return Boolean(
        error
        && typeof error === "object"
        && (error as { code?: unknown }).code === "run_cancelled",
    );
}

function commandSentTerminalRunEvent(error: unknown): boolean {
    if (!error || typeof error !== "object") return false;
    const details = (error as { details?: unknown }).details;
    return Boolean(
        details
        && typeof details === "object"
        && (details as { terminalRunEventSent?: unknown }).terminalRunEventSent === true,
    );
}

export interface ProjectGraphIndexRow {
    path: string;
    name: string;
    type: "event" | "function";
    revision?: number;
    functionRevision?: number;
    functionSignature?: import('@/shared/types/dto/editorMutation').FunctionSignatureDto;
}

export interface ProjectWorksheetIndexRow {
  id: string;
  name: string;
  databaseId: string;
  chartType: string;
}

export interface ProjectVariableIndexRow {
  id: string;
  revision: number;
  name: string;
  dataType: import('@/shared/types/domain').DataType;
  dataValue: import('@/shared/types/domain').DataValue;
  description: string;
  scope: import('@/shared/types/domain/variable').VariableScope;
  tags: string[];
  ownerGraphPath?: string | null;
  ownerGraphName?: string | null;
  ownerGraphKind?: 'event' | 'function' | null;
}

export interface ProjectActivationResult {
  path: string;
  projectInstanceId: string;
  activationRevision: number;
}

export interface ProjectIndexRow {
  projectInstanceId: string;
  projectName: string;
  appVersion: string;
  exportTime: string;
  publicationRevision: number;
  history: HistoryStatusDto;
  graphs: ProjectGraphIndexRow[];
  worksheets?: ProjectWorksheetIndexRow[];
  variables?: ProjectVariableIndexRow[];
}

/**
 * 将后端 Graph 数据转换为前端格式（供 connectPins 等复用）
 */
export function toFrontendGraph(data: GraphInstanceDTO): Graph {
    logger.app.trace(`[toFrontendGraph] Input data: ${JSON.stringify(data)}`, 'ProjectService');
    const graph = graphDataToDomainGraph(graphInstanceDtoToGraphData(data));
    logger.app.trace(
        `[toFrontendGraph] Converted: nodes=${graph.nodes.length}, pins=${graph.pins.length}, connections=${graph.connections.connections.length}`,
        'ProjectService',
    );
    return graph;
}

// ==================== 项目状态管理 API ====================

export type RevealProjectResourceRequest = {
  kind: "graph" | "database" | "worksheet";
  resourceId: string;
};

export class ProjectService {
    // ==================== 项目级操作 ====================

    /**
     * 分阶段加载第一步：获取 databases + variables（含 schema）
     */
    static async getDatabasesVariables(projectInstanceId: string): Promise<{
        databases: Record<string, unknown>;
        variables: Record<string, unknown>;
    }> {
        const data = await invoke<{ databases: Record<string, unknown>; variables: Record<string, unknown> }>(
            "get_project_databases_variables",
            { projectInstanceId },
        );
        return { databases: data.databases || {}, variables: data.variables || {} };
    }

    /**
     * 获取当前项目路径
     */
    static async getProjectPath(projectInstanceId: string): Promise<string | null> {
        return await invoke("get_project_path", { projectInstanceId });
    }

    static async getProjectIndex(projectInstanceId: string): Promise<ProjectIndexRow> {
        return await invoke("get_project_index", { projectInstanceId });
    }

    /**
     * 新建项目（清空当前状态）
     */
    static async newProject(): Promise<void> {
        await invoke("new_project");
    }

    static async defaultProjectParentDirectory(): Promise<string> {
        return await invoke("default_project_parent_directory");
    }

    static async validateNewProjectPath(path: string): Promise<ProjectPathValidation> {
        return await invoke("validate_new_project_path", { path });
    }

    static async createProject(
        name: string,
        path: string,
        operationId: string,
    ): Promise<LifecycleMutationResultDto> {
        return await invoke("create_project", { name, path, operationId });
    }

    static async listRegisteredProjects(): Promise<ProjectRecordRow[]> {
        return await invoke("list_registered_projects");
    }

    static async pickProjectScanDirectory(title?: string): Promise<string | null> {
        const selected = await open({
            directory: true,
            multiple: false,
            title,
        });
        if (!selected || Array.isArray(selected)) return null;
        return selected as string;
    }

    static async cancelProjectPickerTask(): Promise<void> {
        await invoke("cancel_project_picker_task");
    }

    static async cleanupInvalidRegisteredProjects(
        onProgress?: (event: ProjectCleanupProgressEvent) => void,
    ): Promise<CleanupInvalidProjectsResult> {
        const channel = trackChannel(new Channel<ProjectCleanupProgressEvent>());
        channel.onmessage = (event) => {
            onProgress?.(event);
        };
        try {
            return await invoke("cleanup_invalid_registered_projects", {
                onProgress: channel,
            });
        } finally {
            untrackChannel(channel);
        }
    }

    static async scanProjectsInDirectory(
        directory: string,
        onProgress?: (event: ProjectScanProgressEvent) => void,
    ): Promise<ScanProjectsResult> {
        const channel = trackChannel(new Channel<ProjectScanProgressEvent>());
        channel.onmessage = (event) => {
            onProgress?.(event);
        };
        try {
            return await invoke("scan_projects_in_directory", {
                directory,
                onProgress: channel,
            });
        } finally {
            untrackChannel(channel);
        }
    }

    static async registerProject(name: string, path: string): Promise<ProjectRecordRow> {
        return await invoke("register_project", { name, path });
    }

    static async removeRegisteredProject(id: string): Promise<void> {
        await invoke("remove_registered_project", { id });
    }

    static async deleteRegisteredProjectFiles(
        id: string,
        expectedActiveProjectInstanceId: string | null,
        operationId: string,
    ): Promise<LifecycleMutationResultDto> {
        return await invoke("delete_registered_project_files", {
            id,
            expectedActiveProjectInstanceId,
            operationId,
        });
    }

    static async toggleRegisteredProjectFavorite(id: string): Promise<boolean> {
        return await invoke("toggle_registered_project_favorite", { id });
    }

    /**
     * 弹出对话框选择 metadata.yssbi；用户取消时返回 null。
     */
    static async pickProjectMetadataFile(): Promise<string | null> {
        const selected = await open({
            multiple: false,
            filters: [{ name: "YssBI Project", extensions: ["yssbi"] }],
        });
        if (!selected || Array.isArray(selected)) return null;
        return selected as string;
    }

    /**
     * 从文件加载项目到状态管理器
     * 前端只传路径，后端负责加载；加载完成后会发出 ProjectLoaded 事件，前端通过 loadProject 刷新 store
     */
    static async loadProjectToState(path: string): Promise<ProjectActivationResult> {
        return await invoke("load_project", { path });
    }

    /**
     * Flush the current file-backed project to disk.
     */
    static async flushProject(
        projectInstanceId: string,
        operationId: string,
    ): Promise<import('@/shared/types/dto').ProjectSaveResultDto> {
        try {
            return await invoke("flush_project", { projectInstanceId, operationId });
        } catch (e) {
            logger.app.error(`Failed to flush project: ${e instanceof Error ? e.message : String(e)}`, 'ProjectService');
            throw e;
        }
    }

    /** 项目根目录的父路径（用于另存为默认目录） */
    static projectParentDirectory(metadataOrRootPath: string): string {
        const normalized = metadataOrRootPath.replace(/\\/g, "/");
        const root = normalized.replace(/\/metadata\.yssbi$/i, "");
        const idx = root.lastIndexOf("/");
        return idx > 0 ? root.slice(0, idx) : root;
    }

    /**
     * 另存为：选择空目录，复制当前项目并切换工作路径。
     */
    static async saveProjectAs(
        projectInstanceId: string,
        operationId: string,
    ): Promise<LifecycleMutationResultDto | null> {
        try {
            const currentPath = await this.getProjectPath(projectInstanceId);
            if (!currentPath) {
                throw new Error("项目尚未加载");
            }

            const selected = await open({
                directory: true,
                multiple: false,
                title: "项目另存为",
                defaultPath: this.projectParentDirectory(currentPath) || undefined,
            });
            if (!selected || Array.isArray(selected)) return null;

            const validation = await this.validateNewProjectPath(selected);
            if (!validation.ok) {
                throw new Error(validation.message ?? "项目路径无效");
            }

            return await invoke<LifecycleMutationResultDto>("save_project_as", {
                path: selected,
                projectInstanceId,
                operationId,
            });
        } catch (e) {
            logger.app.error(`Failed to save project as: ${e instanceof Error ? e.message : String(e)}`, 'ProjectService');
            throw e;
        }
    }
    /** Execute one graph document and drain its streamed run events. */
    static async executeGraphDocument(
        graphPath: string,
        onEvent?: (event: RunEvent) => void,
    ): Promise<ExecuteGraphResultDto> {
        const { channel, waitForStreamEnd } = bindExecutionEventChannel(onEvent);
        try {
            let result: ExecuteGraphResultDto;
            try {
                result = await invoke<ExecuteGraphResultDto>(
                    "execute_graph_document",
                    { graphPath, onEvent: channel },
                );
            } catch (error) {
                if (commandSentTerminalRunEvent(error)) {
                    try {
                        await waitForStreamEnd();
                    } catch {
                        // Drain failures must not replace the backend command classification.
                    }
                }
                throw error;
            }
            await waitForStreamEnd();
            return result;
        } finally {
            untrackChannel(channel);
        }
    }

    static async cancelGraphRun(runId: string): Promise<boolean> {
        return invoke<boolean>("cancel_graph_run", { runId });
    }

    static async clearGraphExecutionArtifacts(graphPath: string): Promise<void> {
        await invoke("clear_graph_execution_artifacts", { graphPath });
    }

    static async revealProjectResource(
        projectInstanceId: string,
        request: RevealProjectResourceRequest,
    ): Promise<void> {
        const path = await invoke<string>("get_project_resource_path", {
            projectInstanceId,
            kind: request.kind,
            resourceId: request.resourceId,
        });
        await revealItemInDir(path);
    }

    static async revealProjectPath(projectPath: string): Promise<void> {
        await revealItemInDir(projectPath);
    }

}
