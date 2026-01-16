import "./App.css";
import InfiniteCanvas from "./components/canvas/Canvas";
import Sidebar from "./components/Sidebar";
import { DragProvider } from "./components/drag/DragProvider";
import { DragLayer } from "./components/drag/DragLayer";
import { CanvasProvider } from "./components/canvas/CanvasProvider";

export default function App() {
  return (
    <DragProvider>
      <div
        className="flex w-full h-screen"
        onContextMenu={(e) => e.preventDefault()} // 禁用默认菜单
      >
        {/* 左侧 Sidebar */}
        <Sidebar />
        {/* 右侧 Canvas */}
        <div className="flex-1 relative">
          <CanvasProvider>
            <InfiniteCanvas />
          </CanvasProvider>
        </div>
        <DragLayer />
      </div>
    </DragProvider>
  );
}
