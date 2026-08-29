import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import 'dockview-react/dist/styles/dockview.css';
import './workbench-dockview.css';
// 开发期：安装 Tauri IPC 的 HMR 清理（生产构建会被 tree-shake）
import "@/services/devHmrIpc";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
    // <App />
  );
