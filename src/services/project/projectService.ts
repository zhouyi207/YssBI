import { save, open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { GraphData, ProjectData, GraphPosition, PinData } from "@/shared/types/domain";

type CanvasState = GraphPosition;

/**
 * 将后端 Graph 数据转换为前端格式
 */
function toFrontendGraph(data: any): GraphData {
    console.log('[toFrontendGraph] Input data:', data);
    
    // 后端返回的结构：
    // - 新格式（GraphInstanceDTO）：{ id, name, graph_type, nodes: [], pins: [], connections: {...}, canvas }
    // - 旧格式（GraphInstance）：{ id, name, kind, data_state: { nodes: {}, pins: {}, connections: {} }, position }
    
    let nodes: any[] = [];
    let pins: any[] = [];
    let connectionsArray: any[] = [];
    
    // 检查是否是新格式（GraphInstanceDTO）
    if (data.nodes && Array.isArray(data.nodes)) {
        // 新格式：nodes 是数组
        
        // 先处理 pins 数组，创建 Pin ID 到 Pin 对象的映射
        const pinMap = new Map<string, any>();
        if (data.pins && Array.isArray(data.pins)) {
            data.pins.forEach((pin: any) => {
                pinMap.set(pin.id, {
                    id: pin.id,
                    nodeId: pin.node_id,
                    name: pin.name,
                    type: pin.data_type || 'any',
                    node_type: pin.data_type || 'any',
                    direction: pin.direction,
                    links: [],
                    isArray: pin.is_array || false,
                    defaultValue: pin.default_value,
                    userValue: pin.user_value,
                });
            });
        }
        
        // 转换节点，并从 pinMap 中获取对应的 Pin
        nodes = data.nodes.map((node: any) => {
            const nodeInputs: any[] = [];
            const nodeOutputs: any[] = [];
            
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
                type: node.node_type,        // deserializeGraph 需要这个字段
                node_type: node.node_type,   // 保持一致性
                category: node.category || [],
                title: node.title,
                inputs: nodeInputs,
                outputs: nodeOutputs,
                ui_style: node.ui_style || 'default',
                description: node.description,
                position: node.position || { x: 0, y: 0 },
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
                            from_pin: value[0],
                            to_pin: value[1]
                        });
                    } else if (typeof value === 'object' && value !== null) {
                        connectionsArray.push(value);
                    }
                }
            }
        }
    } else if (data.data_state) {
        // 旧格式：data_state 包含 nodes, pins, connections
        const dataState = data.data_state;
        
        // 先处理 pins
        const pinMap = new Map<string, any>();
        if (dataState.pins) {
            Object.values(dataState.pins).forEach((pin: any) => {
                pinMap.set(pin.id, {
                    id: pin.id,
                    nodeId: pin.node_id,
                    name: pin.name,
                    type: pin.data_type || 'any',
                    node_type: pin.data_type || 'any',
                    direction: pin.direction,
                    links: [],
                    isArray: pin.is_array || false,
                    defaultValue: pin.default_value,
                    userValue: pin.user_value,
                });
            });
        }
        
        // 转换 nodes（从 HashMap 到数组）
        if (dataState.nodes) {
            nodes = Object.values(dataState.nodes).map((node: any) => {
                const nodeInputs: any[] = [];
                const nodeOutputs: any[] = [];
                
                if (node.inputs && Array.isArray(node.inputs)) {
                    node.inputs.forEach((pinId: string) => {
                        const pin = pinMap.get(pinId);
                        if (pin) nodeInputs.push(pin);
                    });
                }
                
                if (node.outputs && Array.isArray(node.outputs)) {
                    node.outputs.forEach((pinId: string) => {
                        const pin = pinMap.get(pinId);
                        if (pin) nodeOutputs.push(pin);
                    });
                }
                
                return {
                    id: node.id,
                    type: node.node_type,        // deserializeGraph 需要这个字段
                    node_type: node.node_type,   // 保持一致性
                    category: node.category || [],
                    title: node.title,
                    inputs: nodeInputs,
                    outputs: nodeOutputs,
                    ui_style: node.ui_style || 'default',
                    description: node.description,
                    position: node.position || { x: 0, y: 0 },
                };
            });
        }
        
        pins = Array.from(pinMap.values());
        
        // 转换 connections
        const backendConnections = dataState.connections?.connections || {};
        for (const [_key, value] of Object.entries(backendConnections)) {
            if (Array.isArray(value) && value.length === 2) {
                connectionsArray.push({
                    from_pin: value[0],
                    to_pin: value[1]
                });
            } else if (typeof value === 'object' && value !== null) {
                connectionsArray.push(value);
            }
        }
    }
    
    console.log('[toFrontendGraph] Converted:', {
        nodesCount: nodes.length,
        pinsCount: pins.length,
        connectionsCount: connectionsArray.length,
        sampleNode: nodes[0]
    });
    
    return {
        id: data.id,
        name: data.name,
        type: (data.graph_type || data.kind || 'event').toLowerCase() as "event" | "function" | "macro",
        nodes,
        pins,
        connections: { connections: connectionsArray },
        canvas: data.canvas || data.position || { x: 0, y: 0, scale: 1 }
    };
}

/**
 * 将后端 Graph Map 转换为前端 Record
 */
function convertGraphMap(map: Record<string, any>): Record<string, GraphData> {
    const result: Record<string, GraphData> = {};
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
    static async getProjectData(): Promise<ProjectData> {
        console.log('[ProjectService.getProjectState] Invoking get_project_data...');
        const data: any = await invoke("get_project_data");
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
     * 从文件加载项目到状态管理器 - 使用新的 ProjectData 结构
     */
    static async loadProjectToState(path?: string): Promise<{ project: ProjectData; path: string | null } | null> {
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

            const data: any = await invoke("load_project_to_state", { path: filePath });
            return {
                project: {
                    variables: data.variables || {},
                    graphs: convertGraphMap(data.graphs || {}),
                    databases: data.databases || {},
                    metadata: data.metadata || { exportTime: "", appVersion: "" },
                },
                path: filePath,
            };
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
            await invoke("save_project_from_state", { path: filePath });
            return filePath;
        } catch (e) {
            console.error("Failed to save project:", e);
            throw e;
        }
    }

    /**
     * 设置完整的项目数据（用于批量同步）- 使用新的 ProjectData 结构
     */
    static async setProjectData(data: ProjectData, path?: string, emitEvent: boolean = false): Promise<void> {
        // 直接发送新格式数据
        const backendData = {
            variables: data.variables,
            graphs: data.graphs,
            databases: data.databases,
            metadata: data.metadata,
        };
        console.log('[ProjectService.setProjectData] Sending to backend:', {
            variablesCount: Object.keys(backendData.variables).length,
            graphsCount: Object.keys(backendData.graphs).length,
            databasesCount: Object.keys(backendData.databases || {}).length,
            emitEvent,
        });
        await invoke("set_project_data", { data: backendData, path: path || null, emitEvent });
        console.log('[ProjectService.setProjectData] Successfully sent to backend');
    }



    static async updateCanvas(subgraphId: string, canvas: CanvasState): Promise<void> {
        await invoke("update_canvas", { subgraphId, canvas });
    }

    static async updateSubgraphIo(
        subgraphId: string,
        inputs?: PinData[],
        outputs?: PinData[]
    ): Promise<GraphData> {
        const result: any = await invoke("update_subgraph_io", {
            subgraphId,
            inputs: inputs || null,
            outputs: outputs || null,
        });
        return toFrontendGraph(result);
    }

    static async renameSubgraph(subgraphId: string, newName: string): Promise<GraphData> {
        const result: any = await invoke("rename_subgraph", { subgraphId, newName });
        return toFrontendGraph(result);
    }

}
