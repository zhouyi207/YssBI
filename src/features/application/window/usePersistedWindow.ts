import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { WindowStateService } from "@/services/window/windowStateService";
import type { WindowKind } from "@/shared/types/settings";
import { logger } from "@/utils/appLogger";
import { captureWindowGeometryPreservingMaximized } from "./windowGeometryCapture";

/**
 * 在窗口关闭时把当前几何状态写回后端 `window_state.json`。
 *
 * 不在 mount 时重新应用尺寸/位置：
 * - 主窗口由 Rust 端 `apply_main_window_state` 在 `setup` 阶段已应用并 `show()`。
 * - 子窗口由 `createPersistedWindow` 在创建时直接以保存的尺寸/位置启动。
 * 这样可以避免「先以默认尺寸显示，再被前端缩放」的视觉闪烁。
 *
 * 不阻止关闭（不会调用 `event.preventDefault()`），与 `useMenubar` 的 dirty-tab
 * 拦截相互独立、可叠加。
 */
export function usePersistedWindow(kind: WindowKind): void {
    useEffect(() => {
        const win = getCurrentWindow();
        let unlistenClose: (() => void) | null = null;

        const setup = async () => {
            if (win.label !== "main") return;
            try {
                unlistenClose = await win.onCloseRequested(async () => {
                    const next = await captureWindowGeometryPreservingMaximized(
                        win,
                        () => WindowStateService.get(kind),
                    );
                    if (!next) return;
                    try {
                        await WindowStateService.save(kind, next);
                    } catch (e) {
                        logger.app.error(
                            `Failed to persist window state for ${kind}: ${e instanceof Error ? e.message : String(e)}`,
                            "Window",
                        );
                    }
                });
            } catch (e) {
                logger.app.warn(
                    `Failed to attach close listener for ${kind}: ${e instanceof Error ? e.message : String(e)}`,
                    "Window",
                );
            }
        };

        void setup();
        return () => {
            unlistenClose?.();
        };
    }, [kind]);
}
