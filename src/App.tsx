
import { useEffect, useRef, useCallback } from "react";
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
import { SettingsService, WindowSettings } from "./services/settingsService";

export default function App() {
  const rootId = useLayoutStore(s => s.rootId);
  const saveTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // 防抖保存窗口设置
  const saveWindowSettings = useCallback((settings: Partial<WindowSettings>) => {
    if (saveTimeoutRef.current) {
      clearTimeout(saveTimeoutRef.current);
    }
    saveTimeoutRef.current = setTimeout(() => {
      SettingsService.updateWindow(settings).catch(console.error);
    }, 500);
  }, []);

  useEffect(() => {
    // 初始化时从后端同步节点定义
    NODE_REGISTRY.syncFromBackend();
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

  // 监听窗口变化并保存
  useEffect(() => {
    const appWindow = getCurrentWindow();
    let unlistenResize: (() => void) | null = null;
    let unlistenMove: (() => void) | null = null;
    let isMounted = true;

    const setupListeners = async () => {
      // 监听窗口大小变化
      unlistenResize = await appWindow.onResized(async () => {
        if (!isMounted) return;
        try {
          const isMaximized = await appWindow.isMaximized();
          if (isMaximized) {
            saveWindowSettings({ isMaximized: true });
          } else {
            const size = await appWindow.innerSize();
            saveWindowSettings({
              width: size.width,
              height: size.height,
              isMaximized: false,
            });
          }
        } catch (e) {
          // 忽略错误（窗口可能已关闭）
        }
      });

      // 监听窗口位置变化
      unlistenMove = await appWindow.onMoved(async () => {
        if (!isMounted) return;
        try {
          const isMaximized = await appWindow.isMaximized();
          if (!isMaximized) {
            const position = await appWindow.outerPosition();
            saveWindowSettings({
              x: position.x,
              y: position.y,
            });
          }
        } catch (e) {
          // 忽略错误（窗口可能已关闭）
        }
      });
    };

    setupListeners();

    return () => {
      isMounted = false;
      unlistenResize?.();
      unlistenMove?.();
      if (saveTimeoutRef.current) {
        clearTimeout(saveTimeoutRef.current);
      }
    };
  }, [saveWindowSettings]);

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
