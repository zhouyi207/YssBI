import { invoke } from "@tauri-apps/api/core";
import type { ConnectPinsResult, DisconnectPinResult } from '@/shared/types/dto/graphCommands';


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

}
