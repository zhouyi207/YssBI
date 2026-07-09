import { invoke } from "@tauri-apps/api/core";
import type { GraphUndoPatch } from '@/shared/types/dto/graphUndoPatch';

export interface AutoDisconnected {
    fromPin: string;
    toPin: string;
}

export interface ConnectPinsResult {
    fromPin: string;
    toPin: string;
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

/** 连接服务 — 封装所有连接相关的后端调用 */
export class ConnectionService {
    static async connectPins(graphPath: string, pinA: string, pinB: string): Promise<ConnectPinsResult> {
        return await invoke<ConnectPinsResult>("connect_pins", { graphPath, pinA, pinB });
    }

    static async disconnectPin(graphPath: string, pinId: string): Promise<DisconnectPinResult> {
        return await invoke<DisconnectPinResult>("disconnect_pin", { graphPath, pinId });
    }

    static async deleteConnection(graphPath: string, connectionId: string): Promise<void> {
        await invoke("delete_connection", { graphPath, connectionId });
    }

    static async getConnections(graphPath: string): Promise<unknown[]> {
        return await invoke<unknown[]>("get_connections", { graphPath });
    }

    static async deleteConnectionsForPin(graphPath: string, pinId: string): Promise<string[]> {
        return await invoke<string[]>("delete_connections_for_pin", { graphPath, pinId });
    }

    static async deleteConnectionsForNode(graphPath: string, nodeId: string): Promise<string[]> {
        return await invoke<string[]>("delete_connections_for_node", { graphPath, nodeId });
    }
}
