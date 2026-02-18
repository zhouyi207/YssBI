import { invoke } from "@tauri-apps/api/core";

/**
 * Connection 服务 — 封装所有连接相关的后端调用
 *
 * CQRS 模式：前端只发命令，不主动更新 store。
 * 后端处理后发送 ConnectionCreated/ConnectionDeleted/ConnectionsBatchDeleted 事件，
 * 由 ConnectionEventHandler 自动更新 graphDataStore。
 */
export class ConnectionService {
    /**
     * 连接两个 Pin
     *
     * 后端会自动处理：
     * - 输入 pin 只允许 1 条连接（自动断开旧连接 → ConnectionDeleted 事件）
     * - 环路检测
     * - 类型推断
     * - 动态 pin 重建（→ NodePinsUpdated 事件）
     */
    static async connectPins(subgraphId: string, sourcePinId: string, targetPinId: string): Promise<void> {
        await invoke("connect_pins", { subgraphId, sourcePinId, targetPinId });
    }

    /**
     * 断开 Pin 的所有连接（Alt+Click 触发）
     */
    static async disconnectPin(subgraphId: string, pinId: string): Promise<void> {
        await invoke("disconnect_pin", { subgraphId, pinId });
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
