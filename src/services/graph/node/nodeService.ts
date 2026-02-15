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
     * @returns 创建后的节点数据
     */
    static async createNode(subgraphId: string, nodeType: string): Promise<any> {
        console.log('[NodeService.createNode] Creating node:', { subgraphId, nodeType });
        const result = await invoke("create_node", { 
            graphId: subgraphId, 
            nodeType: nodeType 
        });
        console.log('[NodeService.createNode] Node created successfully:', result);
        return result;
    }

    /**
     * 批量创建节点（循环调用单个创建）
     * @param subgraphId 子图ID
     * @param nodeTypes 节点类型列表
     * @returns 创建后的节点列表
     */
    static async createNodes(subgraphId: string, nodeTypes: string[]): Promise<any[]> {
        console.log('[NodeService.createNodes] Creating nodes:', { subgraphId, count: nodeTypes.length });
        const results: any[] = [];
        for (const nodeType of nodeTypes) {
            try {
                const result = await this.createNode(subgraphId, nodeType);
                results.push(result);
            } catch (error) {
                console.error('[NodeService.createNodes] Failed to create node:', nodeType, error);
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
