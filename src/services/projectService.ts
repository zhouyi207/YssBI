import { save, open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { TabState } from "@/views/EditorView/Store/useNodeStore";
import { SubGraphData, ProjectData, CanvasState, PinDefinition } from "@/views/EditorView/Types/canvas";
import { VariableDefinition } from "@/views/EditorView/Types/variables";
import { serializeSubGraph } from "@/views/EditorView/Utils/io";

// ==================== 后端数据结构转换 ====================

// 注意：后端 Rust 使用 #[serde(rename = "type")]，所以 JSON 字段名是 "type"
// 前端和后端的字段名在 JSON 层面是一致的，不需要转换

/**
 * 将前端 SubGraphData 转换为后端格式
 * 目前前后端使用相同的 JSON 字段名，直接返回
 */
function toBackendSubGraphData(data: SubGraphData): SubGraphData {
    return data;
}

/**
 * 将后端 SubGraphData 转换为前端格式
 * 目前前后端使用相同的 JSON 字段名，直接返回
 */
function toFrontendSubGraphData(data: any): SubGraphData {
    return data as SubGraphData;
}

/**
 * 将后端 HashMap 转换为前端 Record
 */
function convertSubGraphMap(map: Record<string, any>): Record<string, SubGraphData> {
    const result: Record<string, SubGraphData> = {};
    for (const [id, data] of Object.entries(map)) {
        result[id] = toFrontendSubGraphData(data);
    }
    return result;
}

// ==================== 项目状态管理 API ====================

export class ProjectService {
    // ==================== 项目级操作 ====================

    /**
     * 获取当前项目状态
     */
    static async getProjectState(): Promise<ProjectData> {
        console.log('[ProjectService.getProjectState] Invoking get_project_data...');
        const data: any = await invoke("get_project_data");
        console.log('[ProjectService.getProjectState] Raw backend data:', data);
        // 后端使用 serde rename，JSON 字段名是 camelCase (globalVariables)
        const result = {
            globalVariables: data.globalVariables || {},
            events: convertSubGraphMap(data.events || {}),
            functions: convertSubGraphMap(data.functions || {}),
            macros: convertSubGraphMap(data.macros || {}),
            dataframes: data.dataframes || {},
            metadata: data.metadata || { exportTime: "", appVersion: "" },
        };
        console.log('[ProjectService.getProjectState] Converted data:', result);
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
     * 从文件加载项目到状态管理器
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
                    globalVariables: data.globalVariables || {},
                    events: convertSubGraphMap(data.events || {}),
                    functions: convertSubGraphMap(data.functions || {}),
                    macros: convertSubGraphMap(data.macros || {}),
                    dataframes: data.dataframes || {},
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
     * 设置完整的项目数据（用于批量同步）
     * 注意：后端使用 serde rename，JSON 字段名使用 camelCase
     */
    static async setProjectData(data: ProjectData, path?: string, emitEvent: boolean = false): Promise<void> {
        // 直接发送，字段名已经匹配（前端和后端都使用 camelCase 的 JSON 字段名）
        const backendData = {
            globalVariables: data.globalVariables,
            events: data.events,
            functions: data.functions,
            macros: data.macros,
            dataframes: data.dataframes,
            metadata: data.metadata,
        };
        console.log('[ProjectService.setProjectData] Sending to backend:', {
            eventsCount: Object.keys(backendData.events).length,
            functionsCount: Object.keys(backendData.functions).length,
            macrosCount: Object.keys(backendData.macros).length,
            globalVariablesCount: Object.keys(backendData.globalVariables).length,
            dataframesCount: Object.keys(backendData.dataframes || {}).length,
            emitEvent,
        });
        await invoke("set_project_data", { data: backendData, path: path || null, emitEvent });
        console.log('[ProjectService.setProjectData] Successfully sent to backend');
    }

    // ==================== Events CRUD ====================

    static async getEvents(): Promise<Record<string, SubGraphData>> {
        const data: Record<string, any> = await invoke("get_events");
        return convertSubGraphMap(data);
    }

    static async getEvent(id: string): Promise<SubGraphData | null> {
        const data: any = await invoke("get_event", { id });
        return data ? toFrontendSubGraphData(data) : null;
    }

    static async createEvent(id: string, data: SubGraphData): Promise<SubGraphData> {
        const result: any = await invoke("create_event", { id, data: toBackendSubGraphData(data) });
        return toFrontendSubGraphData(result);
    }

    static async updateEvent(id: string, data: SubGraphData): Promise<SubGraphData> {
        const result: any = await invoke("update_event", { id, data: toBackendSubGraphData(data) });
        return toFrontendSubGraphData(result);
    }

    static async deleteEvent(id: string): Promise<void> {
        await invoke("delete_event", { id });
    }

    // ==================== Functions CRUD ====================

    static async getFunctions(): Promise<Record<string, SubGraphData>> {
        const data: Record<string, any> = await invoke("get_functions");
        return convertSubGraphMap(data);
    }

    static async getFunction(id: string): Promise<SubGraphData | null> {
        const data: any = await invoke("get_function", { id });
        return data ? toFrontendSubGraphData(data) : null;
    }

    static async createFunction(id: string, data: SubGraphData): Promise<SubGraphData> {
        const result: any = await invoke("create_function", { id, data: toBackendSubGraphData(data) });
        return toFrontendSubGraphData(result);
    }

    static async updateFunction(id: string, data: SubGraphData): Promise<SubGraphData> {
        const result: any = await invoke("update_function", { id, data: toBackendSubGraphData(data) });
        return toFrontendSubGraphData(result);
    }

    static async deleteFunction(id: string): Promise<void> {
        await invoke("delete_function", { id });
    }

    // ==================== Macros CRUD ====================

    static async getMacros(): Promise<Record<string, SubGraphData>> {
        const data: Record<string, any> = await invoke("get_macros");
        return convertSubGraphMap(data);
    }

    static async getMacro(id: string): Promise<SubGraphData | null> {
        const data: any = await invoke("get_macro", { id });
        return data ? toFrontendSubGraphData(data) : null;
    }

    static async createMacro(id: string, data: SubGraphData): Promise<SubGraphData> {
        const result: any = await invoke("create_macro", { id, data: toBackendSubGraphData(data) });
        return toFrontendSubGraphData(result);
    }

    static async updateMacro(id: string, data: SubGraphData): Promise<SubGraphData> {
        const result: any = await invoke("update_macro", { id, data: toBackendSubGraphData(data) });
        return toFrontendSubGraphData(result);
    }

    static async deleteMacro(id: string): Promise<void> {
        await invoke("delete_macro", { id });
    }

    // ==================== Global Variables CRUD ====================

    static async getGlobalVariables(): Promise<Record<string, VariableDefinition>> {
        return await invoke("get_global_variables");
    }

    static async getGlobalVariable(id: string): Promise<VariableDefinition | null> {
        return await invoke("get_global_variable", { id });
    }

    /**
     * 统一创建变量（后端生成 ID）
     * @param subgraphId 子图 ID（可选，null 为全局）
     * @param name 变量名称建议
     * @param dataType 数据类型
     */
    static async createVariable(
        subgraphId: string | null,
        name?: string,
        dataType?: string
    ): Promise<VariableDefinition> {
        return await invoke("create_variable", { subgraphId, name, dataType });
    }

    static async createGlobalVariable(id: string, data: VariableDefinition): Promise<VariableDefinition> {
        return await invoke("create_global_variable", { id, data });
    }

    static async updateGlobalVariable(id: string, data: VariableDefinition): Promise<VariableDefinition> {
        return await invoke("update_global_variable", { id, data });
    }

    static async deleteGlobalVariable(id: string): Promise<void> {
        await invoke("delete_global_variable", { id });
    }

    // ==================== Local Variables CRUD ====================

    static async getLocalVariables(subgraphId: string): Promise<Record<string, VariableDefinition>> {
        return await invoke("get_local_variables", { subgraphId });
    }

    static async createLocalVariable(subgraphId: string, variableId: string, data: VariableDefinition): Promise<VariableDefinition> {
        return await invoke("create_local_variable", { subgraphId, variableId, data });
    }

    static async updateLocalVariable(subgraphId: string, variableId: string, data: VariableDefinition): Promise<VariableDefinition> {
        return await invoke("update_local_variable", { subgraphId, variableId, data });
    }

    static async deleteLocalVariable(subgraphId: string, variableId: string): Promise<void> {
        await invoke("delete_local_variable", { subgraphId, variableId });
    }

    // ==================== Nodes 操作 ====================

    static async getNodes(subgraphId: string): Promise<any[]> {
        return await invoke("get_nodes", { subgraphId });
    }

    static async setNodes(subgraphId: string, nodes: any[]): Promise<void> {
        await invoke("set_nodes", { subgraphId, nodes });
    }

    /**
     * 创建单个节点（后端生成和验证）
     * @param subgraphId 子图ID
     * @param node 节点数据
     * @returns 创建后的节点数据
     */
    static async createNode(subgraphId: string, node: any): Promise<any> {
        console.log('[ProjectService.createNode] Creating node:', { subgraphId, nodeId: node.id, nodeType: node.type });
        const result = await invoke("create_node", { subgraphId, node });
        console.log('[ProjectService.createNode] Node created successfully:', result);
        return result;
    }

    /**
     * 批量创建节点（后端生成ID和修复连接）
     * @param subgraphId 子图ID
     * @param nodes 节点列表（可包含临时ID）
     * @returns 创建后的节点列表（新ID）
     */
    static async createNodes(subgraphId: string, nodes: any[]): Promise<any[]> {
        console.log('[ProjectService.createNodes] Creating nodes:', { subgraphId, count: nodes.length });
        const newNodes: any[] = await invoke("create_nodes", { subgraphId, nodes });
        console.log('[ProjectService.createNodes] Nodes created successfully:', newNodes);
        return newNodes;
    }

    /**
     * 批量创建节点并保留连接（用于复制/粘贴）
     * @param subgraphId 子图ID
     * @param nodes 节点列表
     * @param connections 连接列表
     * @returns 创建后的节点列表（新ID）
     */
    static async createNodesWithConnections(subgraphId: string, nodes: any[], connections: any[]): Promise<any[]> {
        console.log('[ProjectService.createNodesWithConnections] Creating nodes with connections:', { 
            subgraphId, 
            nodesCount: nodes.length, 
            connectionsCount: connections.length 
        });
        const newNodes: any[] = await invoke("create_nodes_with_connections", { 
            subgraphId, 
            nodes, 
            connections 
        });
        console.log('[ProjectService.createNodesWithConnections] Nodes created successfully with connections:', newNodes);
        return newNodes;
    }

    /**
     * 删除单个节点
     * @param subgraphId 子图ID
     * @param nodeId 节点ID
     */
    static async deleteNode(subgraphId: string, nodeId: string): Promise<void> {
        console.log('[ProjectService.deleteNode] Deleting node:', { subgraphId, nodeId });
        await invoke("delete_node", { subgraphId, nodeId });
        console.log('[ProjectService.deleteNode] Node deleted successfully');
    }

    /**
     * 连接两个 Pin
     * @param subgraphId 子图ID
     * @param sourcePinId 源 Pin ID
     * @param targetPinId 目标 Pin ID
     * @returns 更新后的节点列表
     */
    static async connectPins(subgraphId: string, sourcePinId: string, targetPinId: string): Promise<any[]> {
        console.log('[ProjectService.connectPins] Connecting:', { subgraphId, sourcePinId, targetPinId });
        const nodes = await invoke("connect_pins", { subgraphId, sourcePinId, targetPinId });
        console.log('[ProjectService.connectPins] Connection successful');
        return nodes as any[];
    }

    /**
     * 断开 Pin 的所有连接
     * @param subgraphId 子图ID
     * @param pinId Pin ID
     * @returns 更新后的节点列表
     */
    static async disconnectPin(subgraphId: string, pinId: string): Promise<any[]> {
        console.log('[ProjectService.disconnectPin] Disconnecting:', { subgraphId, pinId });
        const nodes = await invoke("disconnect_pin", { subgraphId, pinId });
        console.log('[ProjectService.disconnectPin] Disconnection successful');
        return nodes as any[];
    }

    // ==================== Connection 管理 ====================

    /**
     * 创建连接
     * @param subgraphId 子图ID
     * @param sourcePinId 源 Pin ID（输出）
     * @param targetPinId 目标 Pin ID（输入）
     * @returns 创建的连接对象
     */
    static async createConnection(subgraphId: string, sourcePinId: string, targetPinId: string): Promise<any> {
        console.log('[ProjectService.createConnection] Creating connection:', { subgraphId, sourcePinId, targetPinId });
        const connection = await invoke("create_connection", { subgraphId, sourcePinId, targetPinId });
        console.log('[ProjectService.createConnection] Connection created:', connection);
        return connection;
    }

    /**
     * 删除连接
     * @param subgraphId 子图ID
     * @param connectionId 连接ID
     */
    static async deleteConnection(subgraphId: string, connectionId: string): Promise<void> {
        console.log('[ProjectService.deleteConnection] Deleting connection:', { subgraphId, connectionId });
        await invoke("delete_connection", { subgraphId, connectionId });
        console.log('[ProjectService.deleteConnection] Connection deleted');
    }

    /**
     * 获取所有连接
     * @param subgraphId 子图ID
     * @returns 连接列表
     */
    static async getConnections(subgraphId: string): Promise<any[]> {
        console.log('[ProjectService.getConnections] Getting connections:', { subgraphId });
        const connections = await invoke("get_connections", { subgraphId });
        console.log('[ProjectService.getConnections] Got connections:', connections.length);
        return connections as any[];
    }

    /**
     * 删除 Pin 的所有连接
     * @param subgraphId 子图ID
     * @param pinId Pin ID
     * @returns 被删除的连接ID列表
     */
    static async deleteConnectionsForPin(subgraphId: string, pinId: string): Promise<string[]> {
        console.log('[ProjectService.deleteConnectionsForPin] Deleting connections for pin:', { subgraphId, pinId });
        const removedIds = await invoke("delete_connections_for_pin", { subgraphId, pinId });
        console.log('[ProjectService.deleteConnectionsForPin] Deleted connections:', removedIds);
        return removedIds as string[];
    }

    /**
     * 删除节点的所有连接
     * @param subgraphId 子图ID
     * @param nodeId 节点ID
     * @returns 被删除的连接ID列表
     */
    static async deleteConnectionsForNode(subgraphId: string, nodeId: string): Promise<string[]> {
        console.log('[ProjectService.deleteConnectionsForNode] Deleting connections for node:', { subgraphId, nodeId });
        const removedIds = await invoke("delete_connections_for_node", { subgraphId, nodeId });
        console.log('[ProjectService.deleteConnectionsForNode] Deleted connections:', removedIds);
        return removedIds as string[];
    }


    static async updateCanvas(subgraphId: string, canvas: CanvasState): Promise<void> {
        await invoke("update_canvas", { subgraphId, canvas });
    }

    static async updateSubgraphIo(
        subgraphId: string,
        inputs?: PinDefinition[],
        outputs?: PinDefinition[]
    ): Promise<SubGraphData> {
        const result: any = await invoke("update_subgraph_io", {
            subgraphId,
            inputs: inputs || null,
            outputs: outputs || null,
        });
        return toFrontendSubGraphData(result);
    }

    static async renameSubgraph(subgraphId: string, newName: string): Promise<SubGraphData> {
        const result: any = await invoke("rename_subgraph", { subgraphId, newName });
        return toFrontendSubGraphData(result);
    }

    // ==================== 数据导入 ====================

    /**
     * 从 CSV 导入数据
     */
    static async importCSV(path: string): Promise<any> {
        return await invoke("import_csv", { path });
    }

    /**
     * 删除数据帧
     */
    static async deleteDataFrame(id: string): Promise<void> {
        await invoke("delete_dataframe", { id });
    }

    /**
     * 创建数据帧（手动）
     */
    static async createDataFrame(id: string, data: any): Promise<any> {
        return await invoke("create_dataframe", { id, data });
    }

    /**
     * 获取数据帧行数据
     */
    static async getDataFrameRows(id: string, offset: number, limit: number): Promise<any[][]> {
        return await invoke("get_dataframe_rows", { id, offset, limit });
    }

    // ==================== 执行 ====================

    /**
     * 执行当前项目（从状态管理器获取数据）
     */
    static async execute(): Promise<string[]> {
        return await invoke("execute_graph");
    }

    /**
     * 执行指定的项目数据
     */
    static async executeProject(
        globalVariables: Record<string, any>,
        events: Record<string, SubGraphData>,
        functions: Record<string, SubGraphData>,
        macros: Record<string, SubGraphData>,
        dataframes: Record<string, any> = {}
    ): Promise<string> {
        const project = this.buildProjectData(globalVariables, events, functions, macros, dataframes);
        // 直接使用 project，字段名已经匹配
        const backendData = {
            globalVariables: project.globalVariables,
            events: project.events,
            functions: project.functions,
            macros: project.macros,
            dataframes: project.dataframes,
            metadata: project.metadata,
        };
        const res: string[] = await invoke("execute_project", { data: backendData });
        return res.join("\n");
    }

    // ==================== 兼容旧接口 ====================

    /**
     * Synchronizes the live state from the node store (tabs) back to the persistent collections.
     */
    static syncStoreToCollections(
        tabs: Record<string, TabState>,
        currentEvents: Record<string, SubGraphData>,
        currentFunctions: Record<string, SubGraphData>,
        currentMacros: Record<string, SubGraphData>
    ): {
        nextEvents: Record<string, SubGraphData>;
        nextFunctions: Record<string, SubGraphData>;
        nextMacros: Record<string, SubGraphData>;
        changed: boolean;
    } {
        const nextEvents = { ...currentEvents };
        const nextFunctions = { ...currentFunctions };
        const nextMacros = { ...currentMacros };
        let changed = false;

        Object.keys(tabs).forEach((id) => {
            const { nodes: liveNodes, variables: liveVars } = tabs[id];

            // Identify which collection the tab belongs to
            let targetCollection: Record<string, SubGraphData> | null = null;
            if (nextEvents[id]) targetCollection = nextEvents;
            else if (nextFunctions[id]) targetCollection = nextFunctions;
            else if (nextMacros[id]) targetCollection = nextMacros;

            if (!targetCollection) return;

            const existing = targetCollection[id];
            if (!existing) return;

            // 调试：打印 existing 的 type 字段
            console.log(`[syncStoreToCollections] id=${id}, existing.type=${existing.type}`);

            const subGraph = serializeSubGraph(
                id,
                existing.name,
                existing.type as any,
                liveNodes,
                existing.canvas,
                liveVars,
                existing.inputs || [],
                existing.outputs || []
            );

            // Check for changes (rudimentary check, or just overwrite as intended)
            // For now we overwrite to ensure consistency, optimization can be added if needed
            targetCollection[id] = { ...existing, ...subGraph };
            changed = true;
        });

        return { nextEvents, nextFunctions, nextMacros, changed };
    }

    /**
     * 构建项目数据对象（内存中）
     */
    static buildProjectData(
        globalVariables: Record<string, any>,
        events: Record<string, SubGraphData>,
        functions: Record<string, SubGraphData>,
        macros: Record<string, SubGraphData>,
        dataframes: Record<string, any> = {}
    ): ProjectData {
        return {
            globalVariables,
            events,
            functions,
            macros,
            dataframes,
            metadata: {
                exportTime: new Date().toISOString(),
                appVersion: "0.1.0",
            },
        };
    }

    /**
     * 另存为项目文件（弹出文件选择对话框）- 兼容旧接口
     */
    static async saveProjectAs(
        globalVariables: Record<string, any>,
        events: Record<string, SubGraphData>,
        functions: Record<string, SubGraphData>,
        macros: Record<string, SubGraphData>,
        dataframes: Record<string, any> = {}
    ): Promise<string | null> {
        try {
            const path = await save({ filters: [{ name: "YssBI Project", extensions: ["json"] }] });
            if (path) {
                const project = this.buildProjectData(globalVariables, events, functions, macros, dataframes);
                // 调用后端保存项目
                await invoke("save_project", {
                    path,
                    projectJson: JSON.stringify(project)
                });
                return path;
            }
        } catch (e) {
            console.error("Failed to save project:", e);
            throw e;
        }
        return null;
    }

    /**
     * 保存项目到指定路径 - 兼容旧接口
     */
    static async saveProject(
        path: string,
        globalVariables: Record<string, any>,
        events: Record<string, SubGraphData>,
        functions: Record<string, SubGraphData>,
        macros: Record<string, SubGraphData>,
        dataframes: Record<string, any> = {}
    ): Promise<void> {
        const project = this.buildProjectData(globalVariables, events, functions, macros, dataframes);
        // 调用后端保存项目
        await invoke("save_project", {
            path,
            projectJson: JSON.stringify(project)
        });
    }

    /**
     * 加载项目文件 - 兼容旧接口
     */
    static async loadProject(): Promise<{ project: ProjectData, path: string | null } | null> {
        try {
            // 弹出文件选择对话框
            const selected = await open({
                multiple: false,
                filters: [{ name: "YssBI Project", extensions: ["json"] }]
            });
            if (!selected || Array.isArray(selected)) return null;

            const path = selected as string;
            // 调用后端加载项目
            const project: ProjectData = await invoke("load_project", { path });
            return { project, path };
        } catch (e) {
            console.error("Failed to load project:", e);
            throw e;
        }
    }
}
