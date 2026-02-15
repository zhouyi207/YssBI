import { invoke } from "@tauri-apps/api/core";

export class NodeService {
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
     * @param nodeType 节点类型
     * @param x 节点 X 坐标（可选）
     * @param y 节点 Y 坐标（可选）
     * @returns 创建后的节点 ID
     */
    static async createNode(
        subgraphId: string, 
        nodeType: string,
        x?: number,
        y?: number
    ): Promise<string> {
        console.log('[NodeService.createNode] Creating node:', { subgraphId, nodeType, x, y });
        const nodeId = await invoke<string>("create_node", { 
            graphId: subgraphId, 
            nodeType: nodeType,
            x: x !== undefined ? x : null,
            y: y !== undefined ? y : null,
        });
        console.log('[NodeService.createNode] Node created successfully, ID:', nodeId);
        return nodeId;
    }

    /**
     * 批量创建节点（循环调用单个创建）
     * @param subgraphId 子图ID
     * @param nodeTypes 节点类型列表
     * @param positions 节点位置列表（可选）
     * @returns 创建后的节点 ID 列表
     */
    static async createNodes(
        subgraphId: string, 
        nodeTypes: string[],
        positions?: Array<{ x: number, y: number }>
    ): Promise<string[]> {
        console.log('[NodeService.createNodes] Creating nodes:', { subgraphId, count: nodeTypes.length });
        const results: string[] = [];
        for (let i = 0; i < nodeTypes.length; i++) {
            try {
                const pos = positions?.[i];
                const nodeId = await this.createNode(
                    subgraphId, 
                    nodeTypes[i],
                    pos?.x,
                    pos?.y
                );
                results.push(nodeId);
            } catch (error) {
                console.error('[NodeService.createNodes] Failed to create node:', nodeTypes[i], error);
            }
        }
        console.log('[NodeService.createNodes] Nodes created successfully:', results);
        return results;
    }

    /**
     * 删除单个节点
     * @param subgraphId 子图ID
     * @param nodeId 节点ID
     */
    static async deleteNode(subgraphId: string, nodeId: string): Promise<void> {
        console.log('[NodeService.deleteNode] Deleting node:', { subgraphId, nodeId });
        await invoke("delete_node", { 
            graphId: subgraphId, 
            nodeId: nodeId 
        });
        console.log('[NodeService.deleteNode] Node deleted successfully');
    }

}
