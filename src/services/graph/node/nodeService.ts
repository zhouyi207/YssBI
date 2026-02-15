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

}
