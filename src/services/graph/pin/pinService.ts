import { invoke } from "@tauri-apps/api/core";
import type { DataValueBackend } from "@/shared/types/dto/dataValue";
import { logger } from '@/utils/appLogger';

/**
 * Pin 服务 - 封装所有 Pin 相关的后端调用
 */
export class PinService {
    /**
     * 更新 Pin 的用户设置值
     * @param subgraphId 子图ID
     * @param nodeId 节点ID
     * @param pinId Pin ID
     * @param value DataValue DTO 格式（前端负责类型转换）
     */
    static async updatePinUserValue(
        subgraphId: string,
        nodeId: string,
        pinId: string,
        value: DataValueBackend | { Null: null }
    ): Promise<void> {
        logger.graph.trace(`Updating pin value: subgraphId=${subgraphId}, nodeId=${nodeId}, pinId=${pinId}`, 'PinService');
        await invoke("update_pin_user_value", {
            subgraphId,
            nodeId,
            pinId,
            value
        });
        logger.graph.debug('Pin value updated successfully', 'PinService');
    }

    /**
     * 清除 Pin 的用户设置值（恢复默认值）
     * @param subgraphId 子图ID
     * @param nodeId 节点ID
     * @param pinId Pin ID
     */
    static async clearPinUserValue(
        subgraphId: string,
        nodeId: string,
        pinId: string
    ): Promise<void> {
        logger.graph.trace(`Clearing pin value: subgraphId=${subgraphId}, nodeId=${nodeId}, pinId=${pinId}`, 'PinService');
        await invoke("clear_pin_user_value", {
            subgraphId,
            nodeId,
            pinId
        });
        logger.graph.debug('Pin value cleared successfully', 'PinService');
    }

    /**
     * 获取 Pin 的当前值（包括连接值、用户值、默认值的优先级处理）
     * @param subgraphId 子图ID
     * @param nodeId 节点ID
     * @param pinId Pin ID
     * @returns Pin 的当前值
     */
    static async getPinValue(
        subgraphId: string,
        nodeId: string,
        pinId: string
    ): Promise<any> {
        logger.graph.trace(`Getting pin value: subgraphId=${subgraphId}, nodeId=${nodeId}, pinId=${pinId}`, 'PinService');
        const value = await invoke("get_pin_value", {
            subgraphId,
            nodeId,
            pinId
        });
        logger.graph.debug(`Got pin value for pinId=${pinId}`, 'PinService');
        return value;
    }

    /**
     * 向节点的 Repeatable 槽位追加一个新 Pin
     */
    static async addRepeatablePin(
        subgraphId: string,
        nodeId: string,
        slotIndex: number
    ): Promise<AddRepeatablePinResult> {
        logger.graph.trace(`Adding repeatable pin: subgraphId=${subgraphId}, nodeId=${nodeId}, slotIndex=${slotIndex}`, 'PinService');
        const result = await invoke<AddRepeatablePinResult>("add_repeatable_pin", {
            subgraphId,
            nodeId,
            slotIndex,
        });
        logger.graph.debug(`Repeatable pin added: pinId=${result.pinId}`, 'PinService');
        return result;
    }

    /**
     * 从节点移除一个 Repeatable 槽位的 Pin
     */
    static async removeRepeatablePin(
        subgraphId: string,
        nodeId: string,
        pinId: string
    ): Promise<RemoveRepeatablePinResult> {
        logger.graph.trace(`Removing repeatable pin: subgraphId=${subgraphId}, nodeId=${nodeId}, pinId=${pinId}`, 'PinService');
        const result = await invoke<RemoveRepeatablePinResult>("remove_repeatable_pin", {
            subgraphId,
            nodeId,
            pinId,
        });
        logger.graph.debug(`Repeatable pin removed: pinId=${pinId}`, 'PinService');
        return result;
    }
}

export interface AddRepeatablePinResult {
    pinId: string;
    pin: import("@/shared/types").PinData;
}

export interface RemoveRepeatablePinResult {
    removedPinId: string;
    slotIndex: number;
    pinIndex: number;
    removedConnections: [string, string][];
}
