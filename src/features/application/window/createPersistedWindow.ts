import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { WindowStateService } from "@/services/window/windowStateService";
import type { WindowKind } from "@/shared/types/settings";
import { logger } from "@/utils/appLogger";

export interface PersistedWindowOptions {
    /** 窗口种类，决定从后端 `window_state.json` 读哪份几何状态 */
    kind: WindowKind;
    label: string;
    url: string;
    title: string;
    /** 是否显示原生装饰（默认 false 与项目其他窗口一致） */
    decorations?: boolean;
    /** 是否在创建时立即可见，默认 false 由窗口自身在准备好后调用 show() */
    visible?: boolean;
    /** 当后端中没有保存位置且调用方希望提供初始坐标时使用 */
    fallbackX?: number;
    fallbackY?: number;
}

/**
 * 异步创建一个 `WebviewWindow`，启动尺寸/位置/最大化状态来自后端
 * `window_state.json`。状态读取失败时回退到后端的内置默认值。
 *
 * 调用方应 `await` 本函数；这与「创建窗口本身就是异步操作」一致。
 */
export async function createPersistedWindow(opts: PersistedWindowOptions): Promise<WebviewWindow> {
    let saved;
    try {
        saved = await WindowStateService.get(opts.kind);
    } catch (e) {
        logger.app.warn(
            `Failed to fetch persisted window state for ${opts.kind}: ${e instanceof Error ? e.message : String(e)}`,
            "Window",
        );
        saved = {
            width: 1000,
            height: 700,
            x: null,
            y: null,
            isMaximized: false,
        };
    }

    const x = saved.x ?? opts.fallbackX;
    const y = saved.y ?? opts.fallbackY;

    const config: Record<string, unknown> = {
        url: opts.url,
        title: opts.title,
        width: saved.width,
        height: saved.height,
        decorations: opts.decorations ?? false,
        visible: opts.visible ?? false,
    };
    if (typeof x === "number" && typeof y === "number") {
        config.x = x;
        config.y = y;
    }
    if (saved.isMaximized) {
        config.maximized = true;
    }

    return new WebviewWindow(opts.label, config);
}
