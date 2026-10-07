import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "flexlayout-react/style/combined.css";
import "./workbench-layout.css";
// 开发期：安装 Tauri IPC 的 HMR 清理（生产构建会被 tree-shake）
import "@/services/devHmrIpc";
import { installFrontendLogging } from "@/features/application/observability/frontendLogTransport";

installFrontendLogging();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
  // <App />
);
