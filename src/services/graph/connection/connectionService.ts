import { invoke } from "@tauri-apps/api/core";
import { GraphService } from "@/services/graph/graphService";
import { toFrontendGraph } from "@/services/project/projectService";
import { useGraphDataStore } from "@/features/core/dataStore";

/**
 * Connection 服务 - 封装所有连接相关的后端调用
 */
export class ConnectionService {
    /**
     * 连接两个 Pin（后端 connect_pins + 刷新 graph 到 store）
     * @param subgraphId 子图ID
     * @param sourcePinId 源 Pin ID
     * @param targetPinId 目标 Pin ID
     */
    static async connectPins(subgraphId: string, sourcePinId: string, targetPinId: string): Promise<void> {
        console.log('[ConnectionService.connectPins] Connecting:', { subgraphId, sourcePinId, targetPinId });
        await invoke("connect_pins", { subgraphId, sourcePinId, targetPinId });
        console.log('[ConnectionService.connectPins] Connection successful, refreshing graph...');
        const rawGraph = await GraphService.getGraph(subgraphId);
        const graph = toFrontendGraph(rawGraph);
        useGraphDataStore.getState().addGraphFromData(subgraphId, graph);
    }

    /**
     * 断开 Pin 的所有连接
     * @param subgraphId 子图ID
     * @param pinId Pin ID
     * @returns 更新后的节点列表
     */
    static async disconnectPin(subgraphId: string, pinId: string): Promise<unknown[]> {
        console.log('[ProjectService.disconnectPin] Disconnecting:', { subgraphId, pinId });
        const nodes = await invoke<unknown[]>("disconnect_pin", { subgraphId, pinId });
        console.log('[ProjectService.disconnectPin] Disconnection successful');
        return nodes;
    }

    // ==================== Connection 管理 ====================

    /**
     * 创建连接
     * @param subgraphId 子图ID
     * @param sourcePinId 源 Pin ID（输出）
     * @param targetPinId 目标 Pin ID（输入）
     * @returns 创建的连接对象
     */
    static async createConnection(subgraphId: string, sourcePinId: string, targetPinId: string): Promise<unknown> {
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
    static async getConnections(subgraphId: string): Promise<unknown[]> {
        console.log('[ProjectService.getConnections] Getting connections:', { subgraphId });
        const connections = await invoke<unknown[]>("get_connections", { subgraphId });
        console.log('[ProjectService.getConnections] Got connections:', connections.length);
        return connections;
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
}