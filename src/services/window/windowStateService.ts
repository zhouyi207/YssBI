import { invokeCommand } from "@/services/ipc";
import type { WindowKind, WindowState } from "@/shared/types/settings";

/**
 * 窗口几何状态服务：透传 Tauri 命令读写后端持久化的窗口尺寸/位置/最大化状态。
 *
 * 后端是权威来源，文件落盘在 `<app_config_dir>/window_state.json`。
 * 主窗口在 `tauri::Builder::setup` 阶段已应用过状态，前端无需重复 setSize；
 * 子窗口在创建时直接以 saved 状态启动，避免「先默认尺寸再被前端缩放」的闪烁。
 */
export const WindowStateService = {
    /** 获取所有 kind 的当前几何状态（未保存过的 kind 会返回内置默认值）。 */
    async getAll(): Promise<Record<WindowKind, WindowState>> {
        return await invokeCommand<Record<WindowKind, WindowState>>("get_window_states");
    },

    /** 读取单个 kind 的几何状态。 */
    async get(kind: WindowKind): Promise<WindowState> {
        return await invokeCommand<WindowState>("get_window_state", { kind });
    },

    /** 写入单个 kind 的几何状态并立即落盘。 */
    async save(kind: WindowKind, value: WindowState): Promise<void> {
        await invokeCommand("save_window_state", { kind, value });
    },
};
