/**
 * 开发期（Vite HMR）专用：Tauri IPC 清理。
 *
 * 长生命周期的 `Channel`（如 `execute_project` 的执行事件流、项目扫描/清理进度流）
 * 可能在模块被 Fast Refresh 替换、或页面整页重载时仍由 Rust 侧持续推送，导致
 * 控制台出现无害的 `[TAURI] Couldn't find callback id` 警告。
 *
 * 该模块仅在开发期生效（整段以 `import.meta.hot` 守卫），生产构建会被完全 tree-shake，
 * 因此 release 天然不受影响、控制台干净。
 */
import type { Channel } from "@tauri-apps/api/core";

const activeChannels = new Set<Channel<unknown>>();

/** 登记一个活跃 Channel，便于 HMR dispose 时统一拆除。生产环境为 no-op。 */
export function trackChannel<T>(channel: Channel<T>): Channel<T> {
  if (import.meta.hot) {
    activeChannels.add(channel as unknown as Channel<unknown>);
  }
  return channel;
}

/** 操作结束后注销 Channel。生产环境为 no-op。 */
export function untrackChannel<T>(channel: Channel<T>): void {
  if (import.meta.hot) {
    activeChannels.delete(channel as unknown as Channel<unknown>);
  }
}

if (import.meta.hot) {
  // 局部 HMR：模块被替换时，把仍在监听的 Channel 处理器置空，
  // 避免 Rust 推送的消息回调到已废弃模块的过期闭包。
  import.meta.hot.dispose(() => {
    for (const channel of activeChannels) {
      try {
        (channel as Channel<unknown>).onmessage = () => {};
      } catch {
        /* 忽略：Channel 可能已被回收 */
      }
    }
    activeChannels.clear();
  });

  // 整页重载：JS 上下文已销毁，但 Rust 可能仍向长生命周期 Channel 推送，
  // Tauri 因而打印无害的 "Couldn't find callback id"。仅过滤这一条开发期噪声。
  const globalWindow = window as unknown as { __yssbiTauriCallbackFilter__?: boolean };
  if (!globalWindow.__yssbiTauriCallbackFilter__) {
    globalWindow.__yssbiTauriCallbackFilter__ = true;
    const isBenignCallbackWarning = (args: unknown[]): boolean =>
      typeof args[0] === "string" && args[0].includes("Couldn't find callback id");

    const originalWarn = console.warn.bind(console);
    console.warn = (...args: unknown[]) => {
      if (isBenignCallbackWarning(args)) return;
      originalWarn(...args);
    };

    const originalError = console.error.bind(console);
    console.error = (...args: unknown[]) => {
      if (isBenignCallbackWarning(args)) return;
      originalError(...args);
    };
  }
}
