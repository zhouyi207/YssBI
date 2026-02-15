import { invoke } from "@tauri-apps/api/core";

/**
 * Pin 服务 - 封装所有 Pin 相关的后端调用
 */
export class PinService {
    /**
     * 更新 Pin 的用户设置值
     * @param subgraphId 子图ID
     * @param nodeId 节点ID
     * @param pinId Pin ID
     * @param value 用户设置的值
     */
    static async updatePinUserValue(
        subgraphId: string,
        nodeId: string,
        pinId: string,
        value: any
    ): Promise<void> {
        console.log('[PinService.updatePinUserValue] Updating pin value:', {
            subgraphId,
            nodeId,
            pinId,
            value
        });
        await invoke("update_pin_user_value", {
            subgraphId,
            nodeId,
            pinId,
            value
        });
        console.log('[PinService.updatePinUserValue] Pin value updated successfully');
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
        console.log('[PinService.clearPinUserValue] Clearing pin value:', {
            subgraphId,
            nodeId,
            pinId
        });
        await invoke("clear_pin_user_value", {
            subgraphId,
            nodeId,
            pinId
        });
        console.log('[PinService.clearPinUserValue] Pin value cleared successfully');
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
        console.log('[PinService.getPinValue] Getting pin value:', {
            subgraphId,
            nodeId,
            pinId
        });
        const value = await invoke("get_pin_value", {
            subgraphId,
            nodeId,
            pinId
        });
        console.log('[PinService.getPinValue] Got pin value:', value);
        return value;
    }
}
