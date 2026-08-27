import { WindowStateService } from "@/services/window/windowStateService";
import { createWebviewWindow } from "@/services/platform/webviewWindow";
import type { WindowKind, WindowState } from "@/shared/types/settings";
import { readWindowDecorationsFromSettings } from "@/features/application/window/windowDecorationPolicy";
import { logger } from "@/utils/appLogger";

export type WindowGeometryPolicy =
    | {
        source: "backend";
        kind: WindowKind;
        fallbackX?: number;
        fallbackY?: number;
    }
    | {
        source: "provided";
        state: WindowState;
    };

export interface PersistedWindowOptions {
    geometry: WindowGeometryPolicy;
    label: string;
    url: string;
    title: string;
    /** 是否显示原生装饰；默认读取 appearance.titleBarStyle */
    decorations?: boolean;
    /** 是否在创建时立即可见，默认 false 由窗口自身在准备好后调用 show() */
    visible?: boolean;
}

async function resolveWindowGeometry(policy: WindowGeometryPolicy): Promise<WindowState> {
    if (policy.source === "provided") return policy.state;

    try {
        const saved = await WindowStateService.get(policy.kind);
        return {
            ...saved,
            x: saved.x ?? policy.fallbackX ?? null,
            y: saved.y ?? policy.fallbackY ?? null,
        };
    } catch (e) {
        logger.app.warn(
            `Failed to fetch persisted window state for ${policy.kind}: ${e instanceof Error ? e.message : String(e)}`,
            "Window",
        );
        return {
            width: 1000,
            height: 700,
            x: policy.fallbackX ?? null,
            y: policy.fallbackY ?? null,
            isMaximized: false,
        };
    }
}

/**
 * 异步创建一个 webview 窗口。几何状态由显式 policy 决定：
 * 普通窗口读取后端，独立窗口可提供自己的 per-label 状态。
 *
 * 调用方应 `await` 本函数；这与「创建窗口本身就是异步操作」一致。
 */
export async function createPersistedWindow(opts: PersistedWindowOptions): Promise<void> {
    const saved = await resolveWindowGeometry(opts.geometry);

    const result = await createWebviewWindow({
        label: opts.label,
        url: opts.url,
        title: opts.title,
        width: saved.width,
        height: saved.height,
        decorations: opts.decorations ?? readWindowDecorationsFromSettings(),
        visible: opts.visible ?? false,
        ...(typeof saved.x === "number" && typeof saved.y === "number"
            ? { x: saved.x, y: saved.y }
            : {}),
        ...(saved.isMaximized ? { maximized: true } : {}),
    });
    if (!result.ok) throw new Error(result.failure.code);
}
