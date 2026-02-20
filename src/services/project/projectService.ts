import { save, open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { Graph, ProjectData, GraphPosition, Pin } from "@/shared/types/domain";
import type { GraphInstanceDTO, ProjectDataDTO } from "@/shared/types/dto";

type CanvasState = GraphPosition;

/**
 * 将后端 Graph 数据转换为前端格式（供 connectPins 等复用）
 */
export function toFrontendGraph(data: GraphInstanceDTO): Graph {
    console.log('[toFrontendGraph] Input data:', data);
    
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
                // 实例参数：刷新后必须保留，否则 get_variable/set_variable 无法从 variable store 响应式读取名称
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
    
    console.log('[toFrontendGraph] Converted:', {
        nodesCount: nodes.length,
        pinsCount: pins.length,
        connectionsCount: connectionsArray.length,
        sampleNode: nodes[0]
    });
    
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
        console.log('[ProjectService.getProjectState] Invoking get_project_data...');
        const data = await invoke<ProjectDataDTO>("get_project_data");
        console.log('[ProjectService.getProjectState] Raw backend data:', JSON.stringify(data));
        
        // 新格式：直接使用 variables, graphs, databases
        const result: ProjectData = {
            variables: data.variables || {},
            graphs: convertGraphMap(data.graphs || {}),
            databases: data.databases || {},
            metadata: data.metadata || { exportTime: "", appVersion: "" },
        };
        
        console.log('[ProjectService.getProjectState] Converted data:', {
            variablesCount: Object.keys(result.variables).length,
            graphsCount: Object.keys(result.graphs).length,
            databasesCount: Object.keys(result.databases).length,
            graphs: result.graphs
        });
        
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
            console.error("Failed to load project:", e);
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
            console.error("Failed to save project:", e);
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
     * 执行项目（从 event_begin 节点开始执行所有 Event 图）
     * @returns { executedGraphs, logs }
     */
    static async executeProject(): Promise<{ executedGraphs: number; logs: string[] }> {
        const res = await invoke<{ executedGraphs: number; logs: string[] }>("execute_project");
        return res;
    }

}
