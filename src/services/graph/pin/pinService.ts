import { invoke } from "@tauri-apps/api/core";
import type { DataValueBackend } from "@/shared/types/dto/dataValue";
import { logger } from '@/utils/appLogger';
import type { AddRepeatablePinResult, RemoveRepeatablePinResult } from '@/shared/types/dto/graphCommands';

/** Pin 服务 - 封装所有 Pin 相关的后端调用 */
export class PinService {
    static async updatePinUserValue(
        graphPath: string,
        nodeId: string,
        pinId: string,
        value: DataValueBackend | { Null: null }
    ): Promise<void> {
        logger.graph.trace(`Updating pin value: graphPath=${graphPath}, nodeId=${nodeId}, pinId=${pinId}`, 'PinService');
        await invoke("update_pin_user_value", {
            graphPath,
            nodeId,
            pinId,
            value
        });
        logger.graph.debug('Pin value updated successfully', 'PinService');
    }

    static async clearPinUserValue(
        graphPath: string,
        nodeId: string,
        pinId: string
    ): Promise<void> {
        logger.graph.trace(`Clearing pin value: graphPath=${graphPath}, nodeId=${nodeId}, pinId=${pinId}`, 'PinService');
        await invoke("clear_pin_user_value", {
            graphPath,
            nodeId,
            pinId
        });
        logger.graph.debug('Pin value cleared successfully', 'PinService');
    }

    static async addRepeatablePin(
        graphPath: string,
        nodeId: string,
        slotIndex: number
    ): Promise<AddRepeatablePinResult> {
        logger.graph.trace(`Adding repeatable pin: graphPath=${graphPath}, nodeId=${nodeId}, slotIndex=${slotIndex}`, 'PinService');
        const result = await invoke<AddRepeatablePinResult>("add_repeatable_pin", {
            graphPath,
            nodeId,
            slotIndex,
        });
        logger.graph.debug(`Repeatable pin added: pinId=${result.pinId}`, 'PinService');
        return result;
    }

    static async removeRepeatablePin(
        graphPath: string,
        nodeId: string,
        pinId: string
    ): Promise<RemoveRepeatablePinResult> {
        logger.graph.trace(`Removing repeatable pin: graphPath=${graphPath}, nodeId=${nodeId}, pinId=${pinId}`, 'PinService');
        const result = await invoke<RemoveRepeatablePinResult>("remove_repeatable_pin", {
            graphPath,
            nodeId,
            pinId,
        });
        logger.graph.debug(`Repeatable pin removed: pinId=${pinId}`, 'PinService');
        return result;
    }
}
