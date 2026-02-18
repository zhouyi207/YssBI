import { invoke } from "@tauri-apps/api/core";

export class NodeService {
   // ==================== Nodes 操作 ====================

    static async getNodes(subgraphId: string): Promise<unknown[]> {
        return await invoke<unknown[]>("get_nodes", { subgraphId });
    }

    static async setNodes(subgraphId: string, nodes: unknown[]): Promise<void> {
        await invoke("set_nodes", { subgraphId, nodes });
    }

    /**
     * 创建单个节点（后端生成和验证）
     * @param subgraphId 子图ID
     * @param nodeType 节点类型
     * @param x 节点 X 坐标（可选）
     * @param y 节点 Y 坐标（可选）
     * @param params 实例参数（variableId、subGraphId 等）
     * @returns 创建后的节点 ID
     */
    static async createNode(
        subgraphId: string, 
        nodeType: string,
        x?: number,
        y?: number,
        params?: {
            variableId?: string;
            variableName?: string;
            variableType?: string;
            subGraphId?: string;
            dataframeId?: string;
            columnName?: string;
            columnType?: string;
        }
    ): Promise<string> {
        console.log('[NodeService.createNode] Creating node:', { subgraphId, nodeType, x, y, params });
        const nodeId = await invoke<string>("create_node", { 
            graphId: subgraphId, 
            nodeType: nodeType,
            x: x !== undefined ? x : null,
            y: y !== undefined ? y : null,
            params: params ?? null,
        });
        console.log('[NodeService.createNode] Node created successfully, ID:', nodeId);
        return nodeId;
    }

    /**
     * 批量创建节点（单次 IPC 调用，后端一次性创建并发出 NodesBatchCreated 事件）
     */
    static async batchCreateNodes(
        graphId: string,
        requests: Array<{
            nodeType: string;
            x?: number;
            y?: number;
            params?: {
                variableId?: string;
                variableName?: string;
                variableType?: string;
                subGraphId?: string;
                dataframeId?: string;
                columnName?: string;
                columnType?: string;
            };
        }>
    ): Promise<string[]> {
        if (requests.length === 0) return [];
        return await invoke<string[]>("batch_create_nodes", {
            graphId,
            requests: requests.map(r => ({
                nodeType: r.nodeType,
                x: r.x ?? null,
                y: r.y ?? null,
                params: r.params ?? null,
            })),
        });
    }

    /**
     * 删除单个节点
     */
    static async deleteNode(graphId: string, nodeId: string): Promise<void> {
        await invoke("delete_node", { graphId, nodeId });
    }

    /**
     * 批量删除节点（单次 IPC 调用，后端一次性删除并发出 NodesBatchDeleted 事件）
     */
    static async batchDeleteNodes(graphId: string, nodeIds: string[]): Promise<void> {
        if (nodeIds.length === 0) return;
        await invoke("batch_delete_nodes", { graphId, nodeIds });
    }

    /**
     * 批量更新节点位置（拖拽结束时调用，CQRS 模式）
     * @param graphId 子图 ID
     * @param updates 节点位置更新列表
     */
    static async updateNodePositions(
        graphId: string,
        updates: Array<{ nodeId: string; x: number; y: number }>
    ): Promise<void> {
        if (updates.length === 0) return;
        await invoke("update_node_positions", { graphId, updates });
    }

}
