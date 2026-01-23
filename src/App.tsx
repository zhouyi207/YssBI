
import { useEffect } from "react";
import "./App.css";
import ActivityBar from "./components/Editor/Layout/ActivityBar";
import { DragProvider } from "./components/Editor/Context/DragProvider";
import { DragLayer } from "./components/Editor/Layout/DragLayer";
import { CanvasProvider } from "./components/Editor/Context/CanvasProvider";
import Menubar from "./components/Editor/Layout/Menubar";
import { NODE_REGISTRY } from "./components/Editor/Nodes/registry";
import { UIProvider } from "./components/Editor/Context/UIProvider";
import { ThemeProvider } from "./components/Editor/Context/ThemeContext";
import { Workspace } from "./components/Editor/Layout/Workspace";
import { useLayoutStore } from "./store/layoutStore";

export default function App() {
  const rootId = useLayoutStore(s => s.rootId);

  useEffect(() => {
    // 初始化时从后端同步节点定义
    NODE_REGISTRY.syncFromBackend();
  }, []);

  return (
    <ThemeProvider>
      <UIProvider>
        <DragProvider>
          <CanvasProvider>
            <div
              className="flex flex-col w-full h-screen bg-[var(--workbench-bg)]"
              onContextMenu={(e) => e.preventDefault()} // 禁用默认菜单
            >
              {/* 顶部菜单栏 */}
              <Menubar />

              <div className="flex flex-1 overflow-hidden">
                {/* 固定的活动栏 */}
                <ActivityBar />

                {/* 核心工作区：由 layoutStore 驱动所有布局节点（侧边栏、主编辑区、详情栏） */}
                <Workspace nodeId={rootId} />
              </div>
              <DragLayer />
            </div>
          </CanvasProvider>
        </DragProvider>
      </UIProvider>
    </ThemeProvider>
  );
}
