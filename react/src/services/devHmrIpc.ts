/**
 * 开发期（Vite HMR）专用：Tauri IPC 清理。
 *
 * 长生命周期的 `Channel`（如 `execute_graph_draft` 的运行事件流、项目扫描/清理进度流）
 * 可能在模块被 Fast Refresh 替换、或页面整页重载时仍由 Rust 侧持续推送，导致
 * 控制台出现无害的 `[TAURI] Couldn't find callback id` 警告。
 *
 * HMR 清理与警告过滤仅在 `import.meta.hot` 下触发；正常完成的操作会主动注销 Channel。
 */
import type { Channel } from "@tauri-apps/api/core";
import {
  clearChannelMessageHandler,
  installTauriCallbackWarningFilterOnce,
} from "@/shared/platform/tauriWebview";

const activeChannels = new Map<object, (() => void) | undefined>();

/** Register an active channel and any pending waiter that HMR must settle. */
export function trackChannel<T>(channel: Channel<T>, onDispose?: () => void): Channel<T> {
  if (import.meta.hot || onDispose) {
    activeChannels.set(channel, onDispose);
  }
  return channel;
}

/** 操作结束后释放 Channel 的清理登记。 */
export function untrackChannel<T>(channel: Channel<T>): void {
  activeChannels.delete(channel);
}

export function disposeTrackedChannelsForHmr(): void {
  const captured = Array.from(activeChannels);
  activeChannels.clear();
  let failure: { value: unknown } | undefined;
  for (const [channel, dispose] of captured) {
    try {
      dispose?.();
    } catch (error) {
      failure ??= { value: error };
    } finally {
      clearChannelMessageHandler(channel);
    }
  }
  if (failure) throw failure.value;
}

if (import.meta.hot) {
  import.meta.hot.dispose(disposeTrackedChannelsForHmr);

  installTauriCallbackWarningFilterOnce();
}
