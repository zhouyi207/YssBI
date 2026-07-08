import { invoke } from "@tauri-apps/api/core";
import type { GraphUndoPatch } from '@/shared/types/dto/graphUndoPatch';

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

export interface DisconnectPinResult {
    removedConnections: RemovedConnection[];
    undoPatch: GraphUndoPatch;
}

/**
 * Connection 服务 — 封装所有连接相关的后端调用
 */
export class ConnectionService {
    static async connectPins(subgraphId: string, pinA: string, pinB: string): Promise<ConnectPinsResult> {
        return await invoke<ConnectPinsResult>("connect_pins", { subgraphId, pinA, pinB });
    }

    static async disconnectPin(subgraphId: string, pinId: string): Promise<DisconnectPinResult> {
        return await invoke<DisconnectPinResult>("disconnect_pin", { subgraphId, pinId });
    }

    static async deleteConnection(subgraphId: string, connectionId: string): Promise<void> {
        await invoke("delete_connection", { subgraphId, connectionId });
    }

    static async getConnections(subgraphId: string): Promise<unknown[]> {
        return await invoke<unknown[]>("get_connections", { subgraphId });
    }

    static async deleteConnectionsForPin(subgraphId: string, pinId: string): Promise<string[]> {
        return await invoke<string[]>("delete_connections_for_pin", { subgraphId, pinId });
    }

    static async deleteConnectionsForNode(subgraphId: string, nodeId: string): Promise<string[]> {
        return await invoke<string[]>("delete_connections_for_node", { subgraphId, nodeId });
    }
}
