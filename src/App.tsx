
import { useEffect, useState } from "react";
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
import { getCurrentWindow, PhysicalSize, PhysicalPosition } from "@tauri-apps/api/window";
import { SettingsService, DEFAULT_WINDOW } from "./services/settingsService";
import { useProjectSync, initProjectSync } from "./components/Editor/Hooks/useProjectSync";
import { installGlobalConsoleForwarder } from "./utils/logger";
import PlotWindow from "./components/Editor/Plot/PlotWindow";
import DataViewWindow from "./components/Editor/DataView/DataViewWindow";

// 安装全局 console 转发器，将所有前端日志转发到后端
installGlobalConsoleForwarder();

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
  const [initialized, setInitialized] = useState(false);

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

  // 初始化：从后端同步节点定义和项目状态
  useEffect(() => {
    const initialize = async () => {
      console.log('[App] Starting initialization...');
      try {
        // 1. 同步节点定义
        console.log('[App] Syncing node definitions from backend...');
        await NODE_REGISTRY.syncFromBackend();
        console.log('[App] Node definitions synced.');
        
        // 2. 从后端同步项目状态
        console.log('[App] Syncing project state from backend...');
        const projectData = await initProjectSync();
        if (projectData) {
          console.log('[App] Project state restored from backend:', {
            events: Object.keys(projectData.events),
            functions: Object.keys(projectData.functions),
            macros: Object.keys(projectData.macros),
          });
        } else {
          console.log('[App] No project data in backend.');
        }
      } catch (e) {
        console.error('[App] Failed to initialize:', e);
      } finally {
        setInitialized(true);
        console.log('[App] Initialization complete.');
      }
    };
    
    initialize();
  }, []);

  // 恢复窗口尺寸和位置，然后显示窗口
  useEffect(() => {
    const restoreWindow = async () => {
      const appWindow = getCurrentWindow();
      
      try {
        const settings = await SettingsService.loadSettings();
        const windowSettings = settings.window;

        // 如果之前是最大化状态，直接最大化
        if (windowSettings.isMaximized) {
          await appWindow.maximize();
        } else {
          // 恢复窗口大小
          if (windowSettings.width && windowSettings.height) {
            await appWindow.setSize(
              new PhysicalSize(windowSettings.width, windowSettings.height)
            );
          }
          // 恢复窗口位置
          if (windowSettings.x !== null && windowSettings.y !== null) {
            await appWindow.setPosition(
              new PhysicalPosition(windowSettings.x, windowSettings.y)
            );
          }
        }
      } catch (error) {
        console.error("Failed to restore window settings:", error);
      }
      
      // 恢复完成后显示窗口
      await appWindow.show();
    };

    restoreWindow();
  }, []);

  // 监听窗口关闭并保存状态
  useEffect(() => {
    const appWindow = getCurrentWindow();
    let unlistenResize: (() => void) | null = null;
    let unlistenMove: (() => void) | null = null;
    let unlistenClose: (() => void) | null = null;

    // 非最大化状态下的窗口尺寸和位置（用于最大化关闭时保留）
    let lastNormalSize = { width: 0, height: 0 };
    let lastNormalPosition = { x: 0, y: 0 };

    const setupListeners = async () => {
      // 初始化时获取当前窗口状态
      try {
        const isMaximized = await appWindow.isMaximized();
        if (!isMaximized) {
          const size = await appWindow.innerSize();
          const position = await appWindow.outerPosition();
          lastNormalSize = { width: size.width, height: size.height };
          lastNormalPosition = { x: position.x, y: position.y };
        }
      } catch (e) {
        // 忽略初始化错误
      }

      // 监听窗口大小变化（仅追踪非最大化尺寸，不保存）
      unlistenResize = await appWindow.onResized(async () => {
        try {
          const isMaximized = await appWindow.isMaximized();
          if (!isMaximized) {
            const size = await appWindow.innerSize();
            lastNormalSize = { width: size.width, height: size.height };
          }
        } catch (e) {
          // 忽略错误
        }
      });

      // 监听窗口位置变化（仅追踪非最大化位置，不保存）
      unlistenMove = await appWindow.onMoved(async () => {
        try {
          const isMaximized = await appWindow.isMaximized();
          if (!isMaximized) {
            const position = await appWindow.outerPosition();
            lastNormalPosition = { x: position.x, y: position.y };
          }
        } catch (e) {
          // 忽略错误
        }
      });

      // 监听窗口关闭请求 - 保存最终状态
      unlistenClose = await appWindow.onCloseRequested(async () => {
        try {
          const isMaximized = await appWindow.isMaximized();
          
          if (isMaximized) {
            // 最大化状态：保存 isMaximized 和之前记录的正常尺寸/位置
            // 如果没有记录到正常尺寸，使用默认值
            const width = lastNormalSize.width > 0 ? lastNormalSize.width : DEFAULT_WINDOW.width;
            const height = lastNormalSize.height > 0 ? lastNormalSize.height : DEFAULT_WINDOW.height;
            
            await SettingsService.updateWindow({
              isMaximized: true,
              width,
              height,
              ...((lastNormalPosition.x !== 0 || lastNormalPosition.y !== 0) && {
                x: lastNormalPosition.x,
                y: lastNormalPosition.y,
              }),
            });
          } else {
            const size = await appWindow.innerSize();
            const position = await appWindow.outerPosition();
            await SettingsService.updateWindow({
              width: size.width,
              height: size.height,
              x: position.x,
              y: position.y,
              isMaximized: false,
            });
          }
        } catch (e) {
          console.error("Failed to save window state on close:", e);
        }
      });
    };

    setupListeners();

    return () => {
      unlistenResize?.();
      unlistenMove?.();
      unlistenClose?.();
    };
  }, []);

  // 等待初始化完成再渲染主界面
  if (!initialized) {
    return (
      <ThemeProvider>
        <div className="flex items-center justify-center w-full h-screen bg-[var(--workbench-bg)]">
          <div className="text-gray-400">加载中...</div>
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
