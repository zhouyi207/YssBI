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
import type { Channel } from '@tauri-apps/api/core';
import { clearChannelMessageHandler, installTauriCallbackWarningFilterOnce } from '@/shared/platform/tauriWebview';

const activeChannels = new Set<object>();

/** 登记一个活跃 Channel，便于 HMR dispose 时统一拆除。生产环境为 no-op。 */
export function trackChannel<T>(channel: Channel<T>): Channel<T> {
  if (import.meta.hot) {
    activeChannels.add(channel);
  }
  return channel;
}

/** 操作结束后注销 Channel。生产环境为 no-op。 */
export function untrackChannel<T>(channel: Channel<T>): void {
  if (import.meta.hot) {
    activeChannels.delete(channel);
  }
}

if (import.meta.hot) {
  import.meta.hot.dispose(() => {
    for (const channel of activeChannels) {
      clearChannelMessageHandler(channel);
    }
    activeChannels.clear();
  });

  installTauriCallbackWarningFilterOnce();
}
