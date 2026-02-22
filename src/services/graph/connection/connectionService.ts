import { invoke } from "@tauri-apps/api/core";

export interface AutoDisconnected {
    fromPin: string;
    toPin: string;
}

export interface ConnectPinsResult {
    fromPin: string;
    toPin: string;
    autoDisconnectedFrom: string | null;
    autoDisconnectedTo: string | null;
    autoDisconnected: AutoDisconnected[];
}

export interface RemovedConnection {
    fromPin: string;
    toPin: string;
}

/**
 * Connection 服务 — 封装所有连接相关的后端调用
 *
 * CQRS 模式：前端只发命令，不主动更新 store。
 * 后端处理后发送 ConnectionCreated/ConnectionDeleted/ConnectionsBatchDeleted 事件，
 * 由 ConnectionEventHandler 自动更新 graphDataStore。
 */
export class ConnectionService {
    /**
     * 连接两个 Pin（无序，后端自动验证方向和兼容性）
     *
     * Returns the actual connection direction and any auto-disconnected connection,
     * used by the command system for undo context.
     */
    static async connectPins(subgraphId: string, pinA: string, pinB: string): Promise<ConnectPinsResult> {
        return await invoke<ConnectPinsResult>("connect_pins", { subgraphId, pinA, pinB });
    }

    /**
     * 断开 Pin 的所有连接（Alt+Click 触发）
     *
     * Returns the list of connections that were removed,
     * used by the command system for undo context.
     */
    static async disconnectPin(subgraphId: string, pinId: string): Promise<RemovedConnection[]> {
        return await invoke<RemovedConnection[]>("disconnect_pin", { subgraphId, pinId });
    }

    /**
     * 删除特定连接（通过 connectionId = "fromPinId->toPinId"）
     */
    static async deleteConnection(subgraphId: string, connectionId: string): Promise<void> {
        await invoke("delete_connection", { subgraphId, connectionId });
    }

    /**
     * 获取子图的所有连接
     */
    static async getConnections(subgraphId: string): Promise<unknown[]> {
        return await invoke<unknown[]>("get_connections", { subgraphId });
    }

    /**
     * 删除 Pin 的所有连接（返回被删除的连接 ID 列表）
     */
    static async deleteConnectionsForPin(subgraphId: string, pinId: string): Promise<string[]> {
        return await invoke<string[]>("delete_connections_for_pin", { subgraphId, pinId });
    }

    /**
     * 删除节点的所有连接（返回被删除的连接 ID 列表）
     */
    static async deleteConnectionsForNode(subgraphId: string, nodeId: string): Promise<string[]> {
        return await invoke<string[]>("delete_connections_for_node", { subgraphId, nodeId });
    }
}
