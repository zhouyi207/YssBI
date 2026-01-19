import { useEffect } from "react";
import "./App.css";
import InfiniteCanvas from "./components/canvas/Canvas";
import Sidebar from "./components/Sidebar";
import { DragProvider } from "./components/drag/DragProvider";
import { DragLayer } from "./components/drag/DragLayer";
import { CanvasProvider } from "./components/canvas/CanvasProvider";
import Menubar from "./components/Menubar";
import { NODE_REGISTRY } from "./components/node/registry";

import { UIProvider } from "./components/ui/UIProvider";

export default function App() {
  useEffect(() => {
    // 初始化时从后端同步节点定义
    NODE_REGISTRY.syncFromBackend();
  }, []);

  return (
    <UIProvider>
      <DragProvider>
        <CanvasProvider>
          <div
            className="flex flex-col w-full h-screen"
            onContextMenu={(e) => e.preventDefault()} // 禁用默认菜单
          >
            {/* 顶部菜单栏 */}
            <Menubar />
            
            <div className="flex flex-1 overflow-hidden">
              {/* 左侧 Sidebar */}
              <Sidebar />
              {/* 右侧 Canvas */}
              <div className="flex-1 relative">
                <InfiniteCanvas />
              </div>
            </div>
            <DragLayer />
          </div>
        </CanvasProvider>
      </DragProvider>
    </UIProvider>
  );
}
