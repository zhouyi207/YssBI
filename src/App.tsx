import { useEffect } from "react";
import "./App.css";
import InfiniteCanvas from "./components/Editor/Canvas/Canvas";
import Sidebar from "./components/Editor/Layout/Sidebar";
import { RightSidebar } from "./components/Editor/Layout/RightSidebar";
import { DragProvider } from "./components/Editor/Context/DragProvider";
import { DragLayer } from "./components/Editor/Layout/DragLayer";
import { CanvasProvider } from "./components/Editor/Context/CanvasProvider";
import Menubar from "./components/Editor/Layout/Menubar";
import { TabBar } from "./components/Editor/Canvas/TabBar";
import { NODE_REGISTRY } from "./components/Editor/Nodes/registry";
import { SettingsView } from "./components/Editor/Settings/SettingsView";
import { useCanvas, GroupContext } from "./components/Editor/Context/CanvasContext";
import { UIProvider } from "./components/Editor/Context/UIProvider";
import { ThemeProvider } from "./components/Editor/Context/ThemeContext";

function EditorContent() {
  const { tabs, activeTabId } = useCanvas();
  const activeTab = tabs.find(t => t.id === activeTabId);

  return (
    <div className="w-full h-full relative">
      {/* Settings View Layer */}
      {activeTab?.type === "setting" && (
        <div className="absolute inset-0 z-10 bg-[var(--workbench-bg)]">
          <SettingsView />
        </div>
      )}

      {/* Canvas Layer - Always mounted to keep shortcut listeners active */}
      <div className={`w-full h-full ${activeTab?.type === "setting" ? "invisible" : "visible"}`}>
        <InfiniteCanvas />
      </div>
    </div>
  );
}

function EditorGroupPane({ groupId }: { groupId: string }) {
  const { activeGroupId, setActiveGroupId, groups } = useCanvas();
  const isActive = activeGroupId === groupId;

  return (
    <GroupContext.Provider value={groupId}>
      <div
        className={`flex-1 flex flex-col min-w-0 overflow-hidden border-r border-[#2b2b2b] last:border-r-0 relative ${isActive ? 'z-10' : ''}`}
        onMouseDownCapture={() => {
          if (!isActive) setActiveGroupId(groupId);
        }}
      >
        {/* Active group indicator border */}
        {isActive && groups.length > 1 && (
          <div className="absolute inset-0 pointer-events-none border border-[var(--accent-color)]/30 z-20" />
        )}
        <TabBar />
        <div className="flex-1 relative min-h-0 overflow-hidden">
          <EditorContent />
        </div>
      </div>
    </GroupContext.Provider>
  );
}

function MainWorkspace() {
  const { groups } = useCanvas();
  return (
    <div className="flex-1 flex min-w-0 overflow-hidden">
      {groups.map((group) => (
        <EditorGroupPane key={group.id} groupId={group.id} />
      ))}
    </div>
  );
}

export default function App() {
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
                {/* 左侧 Sidebar */}
                <Sidebar />
                {/* 右侧 Content (Tab + Canvas) */}
                <MainWorkspace />
                {/* 右侧 Sidebar */}
                <RightSidebar />
              </div>
              <DragLayer />
            </div>
          </CanvasProvider>
        </DragProvider>
      </UIProvider>
    </ThemeProvider>
  );
}
