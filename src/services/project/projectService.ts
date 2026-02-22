import { save, open } from "@tauri-apps/plugin-dialog";
import { invoke, Channel } from "@tauri-apps/api/core";
import type { ExecutionEvent } from "@/shared/types/ui/execution";
import { Graph, ProjectData, GraphPosition, Pin } from "@/shared/types/domain";
import type { GraphInstanceDTO, ProjectDataDTO } from "@/shared/types/dto";
import { logger } from '@/utils/appLogger';

type CanvasState = GraphPosition;

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
                    links: [],
                    containerType: pin.containerType,
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
    const graphType = (typeof rawType === 'string' ? rawType : String(rawType)).toLowerCase() as "event" | "function" | "macro";
    return {
        id: data.id,
        name: data.name,
        type: graphType,
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

    /**
     * 新建项目（清空当前状态）
     */
    static async newProject(): Promise<void> {
        await invoke("new_project");
    }

    /**
     * 从文件加载项目到状态管理器
     * 前端只传路径，后端负责加载；加载完成后会发出 ProjectLoaded 事件，前端通过 syncFromBackend 同步
     */
    static async loadProjectToState(path?: string): Promise<{ path: string } | null> {
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

            await invoke("load_project", { path: filePath });
            return { path: filePath };
        } catch (e) {
            logger.app.error(`Failed to load project: ${e instanceof Error ? e.message : String(e)}`, 'ProjectService');
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
            await invoke("save_project", { path: filePath });
            return filePath;
        } catch (e) {
            logger.app.error(`Failed to save project: ${e instanceof Error ? e.message : String(e)}`, 'ProjectService');
            throw e;
        }
    }

    static async updateCanvas(subgraphId: string, canvas: CanvasState): Promise<void> {
        await invoke("update_canvas", { subgraphId, canvas });
    }

    static async updateSubgraphIo(
        subgraphId: string,
        inputs?: Pin[],
        outputs?: Pin[]
    ): Promise<Graph> {
        const result = await invoke<GraphInstanceDTO>("update_subgraph_io", {
            subgraphId,
            inputs: inputs || null,
            outputs: outputs || null,
        });
        return toFrontendGraph(result);
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
        const channel = new Channel<ExecutionEvent>();
        channel.onmessage = (msg) => {
            onEvent?.(msg);
        };
        const res = await invoke<{ executedGraphs: number; logs: string[] }>(
            "execute_project",
            { onEvent: channel, graphId: graphId ?? null },
        );
        return res;
    }

}
