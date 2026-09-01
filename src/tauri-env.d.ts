/**
 * Tauri / WebView 平台环境增补（拖拽区 CSS、开发期 HMR 全局等）。
 * 由 tsconfig `include: ["src"]` 自动加载；勿在业务组件内 cast。
 */

import "react";

declare global {
  interface Window {
    /**
     * devHmrIpc 开发期：是否已安装 Tauri callback 噪声过滤器。
     * 整页 HMR 重载后 Rust 仍可能向过期 Channel 推送，触发无害的 callback id 警告。
     */
    __yssbiTauriCallbackFilter__?: boolean;
  }
}

declare module "react" {
  interface CSSProperties {
    /**
     * WKWebView / Tauri 自定义标题栏拖拽区。
     * @see https://v2.tauri.app/learn/window-customization/
     */
    WebkitAppRegion?: "drag" | "no-drag";
  }
}

export {};
