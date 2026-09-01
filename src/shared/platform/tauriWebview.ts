/**
 * Tauri WebView 标题栏等平台 glue（拖拽区、交互区指针事件）。
 * 类型增补见 `src/tauri-env.d.ts`。
 */

import type { CSSProperties, PointerEvent } from "react";

/** 标题栏 `data-tauri-drag-region` 内可交互控件：禁止 WebView 窗口拖拽吞掉点击 */
export const TAURI_NO_DRAG_STYLE: CSSProperties = {
  WebkitAppRegion: "no-drag",
};

/** 与 `TAURI_NO_DRAG_STYLE` 配合，阻止指针事件冒泡到 drag region */
export function stopTauriDragPropagation(event: PointerEvent): void {
  event.stopPropagation();
}

/** HMR dispose：拆除 Channel 消息处理器（单点 cast，避免 devHmrIpc 泛型互转） */
export function clearChannelMessageHandler(channel: object): void {
  try {
    const target = channel as { onmessage?: ((message: unknown) => void) | null };
    if ("onmessage" in target) {
      target.onmessage = () => {};
    }
  } catch {
    /* Channel 可能已被回收 */
  }
}

const BENIGN_CALLBACK_WARNING = "Couldn't find callback id";

function isBenignTauriCallbackWarning(args: unknown[]): boolean {
  return typeof args[0] === "string" && args[0].includes(BENIGN_CALLBACK_WARNING);
}

/**
 * 开发期（Vite HMR）过滤 Tauri 在页面重载后推送至过期 callback 的无害警告。
 * 生产构建不调用；由 `devHmrIpc` 在 `import.meta.hot` 分支内触发。
 */
export function installTauriCallbackWarningFilterOnce(): void {
  if (window.__yssbiTauriCallbackFilter__) return;
  window.__yssbiTauriCallbackFilter__ = true;

  const originalWarn = console.warn.bind(console);
  console.warn = (...args: unknown[]) => {
    if (isBenignTauriCallbackWarning(args)) return;
    originalWarn(...args);
  };

  const originalError = console.error.bind(console);
  console.error = (...args: unknown[]) => {
    if (isBenignTauriCallbackWarning(args)) return;
    originalError(...args);
  };
}
