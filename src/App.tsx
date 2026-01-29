
import { useState } from "react";
import "./utils/logger";
import "./App.css";
import ActivityBar from "./components/Editor/Layout/ActivityBar";
import { DragProvider } from "./components/Editor/Context/DragProvider";
import { DragLayer } from "./components/Editor/Layout/DragLayer";
import { CanvasProvider } from "./components/Editor/Context/CanvasProvider";
import Menubar from "./components/Editor/Layout/Menubar";
import { UIProvider } from "./components/Editor/Context/UIProvider";
import { ThemeProvider } from "./components/Editor/Context/ThemeContext";
import { Workspace } from "./components/Editor/Layout/Workspace";
import { useLayoutStore } from "./store/layoutStore";
import { useAppInitialization } from "./components/Editor/Hooks/useAppInitialization";
import { useProjectSync } from "./components/Editor/Hooks/useProjectSync";
import PlotWindow from "./components/Editor/Plot/PlotWindow";
import DataViewWindow from "./components/Editor/DataView/DataViewWindow";


export default function App() {
  // 检查窗口类型
  const isPlotWindow = window.location.hash === '#/plot';
  const isDataViewWindow = window.location.hash === '#/dataview';

  // 如果是 Plot 窗口，只渲染 PlotWindow 组件
  if (isPlotWindow) {
    return (
      <ThemeProvider>
        <PlotWindow />
      </ThemeProvider>
    );
  }

  // 如果是 DataView 窗口，只渲染 DataViewWindow 组件
  if (isDataViewWindow) {
    return (
      <ThemeProvider>
        <DataViewWindow />
      </ThemeProvider>
    );
  }
  const rootId = useLayoutStore(s => s.rootId);

  // 应用初始化
  const { isInitialized, isLoading, error } = useAppInitialization();

  // 订阅后端项目事件，自动同步数据到前端 Store
  useProjectSync({
    enabled: true,
    onProjectLoaded: (_data, path) => {
      console.log('Project loaded from backend:', path);
    },
    onProjectCleared: () => {
      console.log('Project cleared');
    },
    onProjectSaved: (path) => {
      console.log('Project saved to:', path);
    }
  });



  // 等待初始化完成再渲染主界面
  if (!isInitialized) {
    return (
      <ThemeProvider>
        <div className="flex items-center justify-center w-full h-screen bg-[var(--workbench-bg)]">
          {error ? (
            <div className="text-red-400">初始化失败: {error}</div>
          ) : (
            <div className="text-gray-400">加载中...</div>
          )}
        </div>
      </ThemeProvider>
    );
  }

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
