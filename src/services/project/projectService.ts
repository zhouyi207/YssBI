import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { invoke, Channel } from "@tauri-apps/api/core";
import type { ExecutionEvent } from "@/shared/types/ui/execution";
import { Graph, ProjectData, GraphPosition, Pin } from "@/shared/types/domain";
import type { GraphInstanceDTO, ProjectDataDTO } from "@/shared/types/dto";
import { logger } from '@/utils/appLogger';
import { formatErrorMessage } from "@/shared/utils/formatErrorMessage";
import { trackChannel, untrackChannel } from "@/services/devHmrIpc";

type CanvasState = GraphPosition;

export interface ProjectRecordRow {
    id: string;
    name: string;
    path: string;
    createdAt: string;
    lastOpenedAt: string | null;
    isFavorite: boolean;
}

export interface ProjectPathValidation {
    ok: boolean;
    message?: string | null;
}

export interface ScanProjectsResult {
    discovered: number;
    newlyRegistered: number;
    projects: ProjectRecordRow[];
}

export interface CleanupInvalidProjectsResult {
    removed: number;
}

export type ProjectScanProgressEvent =
    | { kind: "scanning" }
    | { kind: "discovered"; count: number }
    | { kind: "registering"; current: number; total: number };

export type ProjectCleanupProgressEvent =
    | { kind: "checking"; current: number; total: number }
    | { kind: "removing"; removed: number; total: number };

export const PICKER_TASK_CANCELLED = "PICKER_TASK_CANCELLED";

/** @deprecated 使用 PICKER_TASK_CANCELLED */
export const SCAN_CANCELLED = PICKER_TASK_CANCELLED;

export function isPickerTaskCancelledError(error: unknown): boolean {
    return formatErrorMessage(error, "") === PICKER_TASK_CANCELLED;
}

/** @deprecated 使用 isPickerTaskCancelledError */
export function isScanCancelledError(error: unknown): boolean {
    return isPickerTaskCancelledError(error);
}

export interface ProjectGraphIndexRow {
    id: string;
    name: string;
    type: "event" | "function";
}

export interface ProjectWorksheetIndexRow {
  id: string;
  name: string;
  databaseId: string;
  chartType: string;
}

export interface ProjectVariableIndexRow {
  id: string;
  name: string;
  dataType: import('@/shared/types/domain').DataType;
  dataValue: import('@/shared/types/domain').DataValue;
  description: string;
  scope: import('@/shared/types/domain/variable').VariableScope;
  tags: string[];
  ownerGraphId?: string | null;
  ownerGraphName?: string | null;
  ownerGraphKind?: 'event' | 'function' | null;
}

export interface ProjectIndexRow {
  projectName: string;
  appVersion: string;
  exportTime: string;
  graphs: ProjectGraphIndexRow[];
  worksheets?: ProjectWorksheetIndexRow[];
  variables?: ProjectVariableIndexRow[];
}

export interface LoadedProjectGraphRow {
    graph: GraphInstanceDTO;
    variables: Record<string, unknown>;
}

/**
 * 将后端 Graph 数据转换为前端格式（供 connectPins 等复用）
 */
export function toFrontendGraph(data: GraphInstanceDTO): Graph {
    logger.app.trace(`[toFrontendGraph] Input data: ${JSON.stringify(data)}`, 'ProjectService');
    
    // 后端返回的结构（GraphInstanceDTO）：{ id, name, type, nodes: [], pins: [], connections: {...}, canvas }
    
    let nodes: Graph["nodes"] = [];
    let pins: Graph["pins"] = [];
    let connectionsArray: { fromPin: string; toPin: string }[] = [];
    
    if (data.nodes && Array.isArray(data.nodes)) {
        // 先处理 pins 数组，创建 Pin ID 到 Pin 对象的映射
        const pinMap = new Map<string, Pin>();
        if (data.pins && Array.isArray(data.pins)) {
            data.pins.forEach((pin) => {
                const pinType = pin.type ?? 'any';
                pinMap.set(pin.id, {
                    id: pin.id,
                    nodeId: pin.nodeId,
                    name: pin.name,
                    type: pinType,
                    direction: pin.direction,
                    containerType: pin.containerType,
                    typeDisplay: pin.typeDisplay,
                    dataType: pin.dataType,
                    optional: pin.optional ?? false,
                    defaultValue: pin.defaultValue,
                    userValue: pin.userValue,
                    ui: pin.ui ? { x: pin.ui.x, y: pin.ui.y, color: pin.ui.color } : undefined,
                });
            });
        }
        
        // 转换节点，并从 pinMap 中获取对应的 Pin
        nodes = data.nodes.map((node) => {
            const nodeInputs: Pin[] = [];
            const nodeOutputs: Pin[] = [];
            
            // 从 node.inputs (Pin IDs) 中获取完整的 Pin 对象
            if (node.inputs && Array.isArray(node.inputs)) {
                node.inputs.forEach((pinId: string) => {
                    const pin = pinMap.get(pinId);
                    if (pin) {
                        nodeInputs.push(pin);
                    }
                });
            }
            
            // 从 node.outputs (Pin IDs) 中获取完整的 Pin 对象
            if (node.outputs && Array.isArray(node.outputs)) {
                node.outputs.forEach((pinId: string) => {
                    const pin = pinMap.get(pinId);
                    if (pin) {
                        nodeOutputs.push(pin);
                    }
                });
            }
            
            return {
                id: node.id,
                nodeType: node.nodeType,
                category: node.category || [],
                title: node.title,
                inputs: nodeInputs,
                outputs: nodeOutputs,
                uiStyle: node.uiStyle ?? 'default',
                description: node.description,
                position: node.position || { x: 0, y: 0 },
                paramsKind: node.paramsKind ?? 'none', // 缺失这个会导致初始化的节点和复制的节点类型不一致
                variableId: node.variableId,
                variableName: node.variableName,
                variableType: node.variableType,
                subGraphId: node.subGraphId,
                dataframeId: node.dataframeId,
            };
        });
        
        pins = Array.from(pinMap.values());
        
        // connections 可能是对象或数组
        if (data.connections) {
            if (Array.isArray(data.connections)) {
                connectionsArray = data.connections;
            } else if (data.connections.connections && Array.isArray(data.connections.connections)) {
                connectionsArray = data.connections.connections;
            } else if (typeof data.connections === 'object') {
                // 将对象转换为数组
                for (const [_key, value] of Object.entries(data.connections)) {
                    if (Array.isArray(value) && value.length === 2) {
                        connectionsArray.push({
                            fromPin: value[0],
                            toPin: value[1]
                        });
                    } else if (typeof value === 'object' && value !== null) {
                        connectionsArray.push(value);
                    }
                }
            }
        }
    }
    
    logger.app.trace(`[toFrontendGraph] Converted: nodes=${nodes.length}, pins=${pins.length}, connections=${connectionsArray.length}`, 'ProjectService');
    
    const rawType = data.type ?? 'event';
    const graphType = (typeof rawType === 'string' ? rawType : String(rawType)).toLowerCase() as "event" | "function";
    return {
        id: data.id,
        name: data.name,
        type: graphType,
        functionInputs: data.functionInputs ?? [],
        functionOutputs: data.functionOutputs ?? [],
        nodes,
        pins,
        connections: { connections: connectionsArray },
        canvas: data.canvas ?? (data as GraphInstanceDTO & { position?: GraphPosition }).position ?? { x: 0, y: 0, scale: 1 }
    };
}

/**
 * 将后端 Graph Map 转换为前端 Record
 */
function convertGraphMap(map: Record<string, GraphInstanceDTO>): Record<string, Graph> {
    const result: Record<string, Graph> = {};
    for (const [id, data] of Object.entries(map)) {
        result[id] = toFrontendGraph(data);
    }
    return result;
}

// ==================== 项目状态管理 API ====================

export type RevealProjectResourceRequest = {
  kind: "graph" | "database" | "worksheet";
  resourceId: string;
};

export class ProjectService {
    // ==================== 项目级操作 ====================

    /**
     * 获取当前项目状态 - 使用新的 ProjectData 结构
     */
    static async getProjectState(): Promise<ProjectData> {
        logger.app.debug('Invoking get_project_data...', 'ProjectService');
        const data = await invoke<ProjectDataDTO>("get_project_data");
        logger.app.trace(`Raw backend data: ${JSON.stringify(data)}`, 'ProjectService');
        
        // 新格式：直接使用 variables, graphs, databases
        const result: ProjectData = {
            variables: data.variables || {},
            graphs: convertGraphMap(data.graphs || {}),
            databases: data.databases || {},
            metadata: data.metadata || { exportTime: "", appVersion: "" },
        };
        
        logger.app.debug(`Converted data: variables=${Object.keys(result.variables).length}, graphs=${Object.keys(result.graphs).length}, databases=${Object.keys(result.databases).length}`, 'ProjectService');
        
        return result;
    }

    /**
     * 获取当前项目数据（getProjectState 的别名，用于兼容）
     */
    static async getProjectData(): Promise<ProjectData> {
        return ProjectService.getProjectState();
    }

    /**
     * 分阶段加载第一步：获取 databases + variables（含 schema）
     */
    static async getDatabasesVariables(): Promise<{
        databases: Record<string, unknown>;
        variables: Record<string, unknown>;
    }> {
        const data = await invoke<{ databases: Record<string, unknown>; variables: Record<string, unknown> }>(
            "get_project_databases_variables"
        );
        return { databases: data.databases || {}, variables: data.variables || {} };
    }

    /**
     * 分阶段加载第二步：获取 graphs，含引用校验结果
     */
    static async getProjectGraphs(): Promise<{
        graphs: Record<string, GraphInstanceDTO>;
        invalidReferences: Record<string, Array<{ nodeId: string; variableId?: string; dataframeId?: string; subGraphId?: string }>>;
    }> {
        const data = await invoke<{
            graphs: Record<string, GraphInstanceDTO>;
            invalidReferences: Record<string, Array<{ nodeId: string; variableId?: string; dataframeId?: string; subGraphId?: string }>>;
        }>("get_project_graphs");
        return {
            graphs: data.graphs || {},
            invalidReferences: data.invalidReferences || {},
        };
    }

    /**
     * 获取当前项目路径
     */
    static async getProjectPath(): Promise<string | null> {
        return await invoke("get_project_path");
    }

    static async getProjectIndex(): Promise<ProjectIndexRow> {
        return await invoke("get_project_index");
    }

    static async loadProjectGraph(graphId: string): Promise<LoadedProjectGraphRow> {
        return await invoke("load_project_graph", { graphId });
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

    static async createProject(name: string, path: string): Promise<ProjectRecordRow> {
        return await invoke("create_project", { name, path });
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

    /** @deprecated 使用 cancelProjectPickerTask */
    static async cancelProjectScan(): Promise<void> {
        return ProjectService.cancelProjectPickerTask();
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

    static async deleteRegisteredProjectFiles(id: string): Promise<void> {
        await invoke("delete_registered_project_files", { id });
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
    static async loadProjectToState(path: string): Promise<{ path: string }> {
        await invoke("load_project", { path });
        return { path };
    }

    /**
     * Flush the current file-backed project to disk.
     */
    static async flushProject(): Promise<void> {
        try {
            await invoke("flush_project");
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
    static async saveProjectAs(): Promise<ProjectRecordRow | null> {
        try {
            const currentPath = await this.getProjectPath();
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

            return await invoke<ProjectRecordRow>("save_project_as", { path: selected });
        } catch (e) {
            logger.app.error(`Failed to save project as: ${e instanceof Error ? e.message : String(e)}`, 'ProjectService');
            throw e;
        }
    }

    static async updateCanvas(subgraphId: string, canvas: CanvasState): Promise<void> {
        await invoke("update_canvas", { subgraphId, canvas });
    }

    static async renameSubgraph(subgraphId: string, newName: string): Promise<Graph> {
        const result = await invoke<GraphInstanceDTO>("rename_subgraph", { subgraphId, newName });
        return toFrontendGraph(result);
    }

    /**
     * 执行指定的 Event 图（通过 Tauri Channel 流式接收执行事件）
     * @param graphId 要执行的 graph ID，传 undefined 则执行所有 Event 图
     */
    static async executeProject(
        onEvent?: (event: ExecutionEvent) => void,
        graphId?: string,
    ): Promise<{ executedGraphs: number; logs: string[] }> {
        const channel = trackChannel(new Channel<ExecutionEvent>());
        channel.onmessage = (msg) => {
            onEvent?.(msg);
        };
        try {
            const res = await invoke<{ executedGraphs: number; logs: string[] }>(
                "execute_project",
                { onEvent: channel, graphId: graphId ?? null },
            );
            return res;
        } finally {
            untrackChannel(channel);
        }
    }

    static async revealProjectResource(request: RevealProjectResourceRequest): Promise<void> {
        const path = await invoke<string>("get_project_resource_path", {
            kind: request.kind,
            resourceId: request.resourceId,
        });
        await revealItemInDir(path);
    }

    static async revealProjectPath(projectPath: string): Promise<void> {
        await revealItemInDir(projectPath);
    }

}
