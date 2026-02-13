import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCanvas } from "@/features/editor";
import { useState, useEffect } from "react";
import { useLayoutStore } from "../../../features/layoutStore/layoutStore";
import { VscLayoutSidebarRight, VscLayoutSidebarRightOff, VscSettingsGear } from "react-icons/vsc";
import { open } from "@tauri-apps/plugin-dialog";
import { uiStore } from "@/features/ui/UIStore";
import { ProjectService } from "../../../services/project/projectService";
import { useProjectStore } from "@/features/project";
import { SettingsService } from "../../../services/settings/settingsService";
import { DEFAULT_WINDOW } from "@/app/appConfig/default";

interface MenuItem {
  label: string;
  shortcut?: string;
  onClick?: () => void;
  type?: 'item' | 'separator';
}

const MenuButton = ({ label, items }: { label: string; items: MenuItem[] }) => {
  const [isOpen, setIsOpen] = useState(false);

  return (
    <div className="relative group">
      <button
        onMouseEnter={() => setIsOpen(true)}
        onMouseLeave={() => setIsOpen(false)}
        className="px-3 py-1 text-sm text-gray-400 hover:text-white hover:bg-[var(--sidebar-bg)] rounded transition-colors"
      >
        {label}
      </button>
      {isOpen && (
        <div
          onMouseEnter={() => setIsOpen(true)}
          onMouseLeave={() => setIsOpen(false)}
          className="absolute left-0 top-full min-w-[180px] w-max bg-[var(--sidebar-bg)] border border-gray-700 rounded shadow-2xl py-1 z-50 backdrop-blur-sm"
        >
          {items.map((item, i) => {
            if (item.type === 'separator' || item.label === '-') {
              return <div key={i} className="my-1 border-t border-gray-700/50 mx-1" />;
            }

            const isDisabled = !item.onClick;

            return (
              <div
                key={i}
                onClick={() => {
                  if (item.onClick) {
                    item.onClick();
                    setIsOpen(false);
                  }
                }}
                className={`px-3 py-1.5 text-[11px] flex items-center justify-between transition-colors whitespace-nowrap gap-10 ${!isDisabled
                  ? "text-gray-300 hover:bg-[var(--accent-color)] hover:text-white cursor-pointer"
                  : "text-gray-600 cursor-default"
                  }`}
              >
                <span className="flex-1">{item.label}</span>
                {item.shortcut ? (
                  <span className={`text-[10px] font-mono ml-4 ${!isDisabled ? 'opacity-40' : 'opacity-20'}`}>{item.shortcut}</span>
                ) : (
                  isDisabled && <span className="text-[9px] opacity-40 ml-4">⏳</span>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};

export function Menubar() {
  const {
    saveGraphAs,
    importGraph,
    saveGraph,
    undo,
    redo,
    copy,
    paste,
    cut,
    deleteSelected,
    canUndo,
    canRedo,
    activeTabId,
    addEvent,
    addFunction,
    addMacro,
    addDataFrame,
  } = useCanvas();

  const openSettings = useLayoutStore(s => s.openSettings);
  const activeEditorGroupId = useLayoutStore(s => s.activeEditorGroupId);
  const splitNode = useLayoutStore(s => s.splitNode);

  // 监听窗口关闭并保存状态
  useEffect(() => {
    const appWindow = getCurrentWindow();
    let unlistenClose: (() => void) | null = null;

    const setupCloseListener = async () => {
      unlistenClose = await appWindow.onCloseRequested(async () => {
        try {
          const isMaximized = await appWindow.isMaximized();

          if (isMaximized) {
            await SettingsService.updateWindow({
              isMaximized: true,
              width: DEFAULT_WINDOW.width,
              height: DEFAULT_WINDOW.height,
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

    setupCloseListener();

    return () => {
      unlistenClose?.();
    };
  }, []);

  const handleImportData = () => {
    uiStore.showImportDialog({
      onSelect: async (type) => {
        if (type === 'csv') {
          try {
            const selected = await open({
              multiple: false,
              filters: [{ name: "CSV File", extensions: ["csv"] }]
            });

            if (selected && !Array.isArray(selected)) {
              uiStore.showToast(`正在从 CSV 导入数据...`, "info");
              const dfData = await ProjectService.importCSV(selected);

              // 更新前端 store
              useProjectStore.getState().addDataFrame(dfData.id, dfData);
              uiStore.showToast(`CSV 数据导入成功: ${dfData.row_count} 行`, "success");
            }
          } catch (error) {
            console.error("Failed to import CSV:", error);
            uiStore.showToast(`CSV 导入失败: ${error}`, "error");
          }
        } else {
          uiStore.showToast(`${type.toUpperCase()} 导入功能开发中...`, "warning");
        }
      }
    });
  };

  const handleSplitRight = () => {
    if (activeEditorGroupId) {
      const node = useLayoutStore.getState().nodes[activeEditorGroupId];
      const activeTab = node?.data?.tabs?.find(t => t.id === node.data?.activeTabId);
      splitNode(activeEditorGroupId, 'row', activeTab?.component || 'GraphEditor');
    }
  };

  const handleSplitDown = () => {
    if (activeEditorGroupId) {
      const node = useLayoutStore.getState().nodes[activeEditorGroupId];
      const activeTab = node?.data?.tabs?.find(t => t.id === node.data?.activeTabId);
      splitNode(activeEditorGroupId, 'col', activeTab?.component || 'GraphEditor');
    }
  };

  const detailNode = useLayoutStore(s => s.nodes['detail']);
  const updateNode = useLayoutStore(s => s.updateNode);
  const isDetailVisible = detailNode?.data?.visible !== false;

  const handleDataView = async () => {
    try {
      const label = `dataview-${Math.random().toString(36).substring(7)}`;
      new WebviewWindow(label, {
        url: "index.html#/dataview",
        title: "Data Viewer",
        width: 1000,
        height: 600,
        decorations: false,
        visible: false, // 初始隐藏，待渲染完毕后再显示
      });
    } catch (error) {
      console.error("Failed to open data view:", error);
      uiStore.showToast("无法打开数据视图窗口", "error");
    }
  };

  const handleOpenLogs = async () => {
    try {
      const label = `logs-${Math.random().toString(36).substring(7)}`;
      new WebviewWindow(label, {
        url: "index.html#/logs",
        title: "Logs",
        width: 1000,
        height: 600,
        decorations: false,
        visible: false, // 初始隐藏，待渲染完毕后再显示
      });
    } catch (error) {
      console.error("Failed to open logs window:", error);
      uiStore.showToast("无法打开日志窗口", "error");
    }
  };

  const toggleDetail = () => {
    updateNode('detail', {
      data: { ...detailNode?.data, visible: !isDetailVisible }
    });
  };

  const openNewWindow = async () => {
    try {
      const label = `window-${Math.random().toString(36).substring(7)}`;
      new WebviewWindow(label, {
        url: "index.html",
        title: "YssBI Node Editor",
        width: 1000,
        height: 800,
        decorations: false,
      });
    } catch (error) {
      console.error("Failed to open new window:", error);
    }
  };

  const fileItems: MenuItem[] = [
    { label: "New Event Graph", shortcut: "Ctrl+N", onClick: () => addEvent() },
    { label: "New Function", onClick: () => addFunction() },
    { label: "New Macro", onClick: () => addMacro() },
    { label: "-" },
    { label: "Open Project...", shortcut: "Ctrl+O", onClick: () => importGraph() },
    { label: "-" },
    { label: "Save Project", shortcut: "Ctrl+S", onClick: activeTabId ? () => saveGraph() : undefined },
    { label: "Save Project As...", shortcut: "Ctrl+Shift+S", onClick: activeTabId ? () => saveGraphAs() : undefined },
  ];

  const editItems: MenuItem[] = [
    { label: "Undo", shortcut: "Ctrl+Z", onClick: (activeTabId && canUndo) ? undo : undefined },
    { label: "Redo", shortcut: "Ctrl+Y", onClick: (activeTabId && canRedo) ? redo : undefined },
    { label: "-" },
    { label: "Cut", shortcut: "Ctrl+X", onClick: activeTabId ? cut : undefined },
    { label: "Copy", shortcut: "Ctrl+C", onClick: activeTabId ? copy : undefined },
    { label: "Paste", shortcut: "Ctrl+V", onClick: activeTabId ? () => paste() : undefined },
    { label: "-" },
    { label: "Delete", shortcut: "Del", onClick: activeTabId ? deleteSelected : undefined },
  ];

  const dataItems: MenuItem[] = [
    { label: "Manage Variables" },
    { label: "Import Data", onClick: handleImportData },
    { label: "Data Viewer", onClick: handleDataView },
    { label: "-" },
    { label: "Schema Viewer" },
  ];

  const windowItems: MenuItem[] = [
    { label: "New Window", onClick: openNewWindow },
    { label: "-" },
    { label: "Split Editor Right", onClick: handleSplitRight },
    { label: "Split Editor Down", onClick: handleSplitDown },
    { label: "-" },
    { label: "Logs", onClick: handleOpenLogs },
    { label: "-" },
    { label: "Reset Layout" },
    { label: "Zoom In", shortcut: "Ctrl++" },
    { label: "Zoom Out", shortcut: "Ctrl+-" },
  ];

  const toolItems: MenuItem[] = [
    { label: "Debugger" },
    { label: "Profiler" },
    { label: "-" },
    { label: "Settings", shortcut: "Ctrl+,", onClick: openSettings },
  ];

  const helpItems: MenuItem[] = [
    { label: "Documentation" },
    { label: "Release Notes" },
    { label: "About" },
  ];

  return (
    <div
      className="menubar-container h-10 bg-[var(--workbench-bg)] border-b border-gray-800 flex items-center z-50 shadow-xl select-none"
      onWheel={(e) => e.stopPropagation()}
      data-tauri-drag-region
    >
      {/* Left: Icon & Brand */}
      <div className="flex items-center gap-2 px-4 pointer-events-none">
        <div className="w-5 h-5 bg-[var(--accent-color)] rounded flex items-center justify-center">
          <span className="text-white font-black text-xs">Y</span>
        </div>
        <div className="text-white font-bold text-sm tracking-tight">
          Yss<span className="text-[var(--accent-color)]">BI</span>
        </div>
      </div>

      {/* Center Left: Menus */}
      <div className="flex items-center gap-1">
        <MenuButton label="File" items={fileItems} />
        <MenuButton label="Edit" items={editItems} />
        <MenuButton label="Data" items={dataItems} />
        <MenuButton label="Window" items={windowItems} />
        <MenuButton label="Tools" items={toolItems} />
        <MenuButton label="Help" items={helpItems} />
      </div>

      <div className="flex-1 min-w-[20px]" data-tauri-drag-region />

      {/* Right side: Window Buttons */}
      <div className="flex items-center h-full">
        {/* Toggle Detail Button */}
        <button
          onClick={toggleDetail}
          className={`w-10 h-10 flex items-center justify-center transition-colors ${isDetailVisible ? 'text-[var(--accent-color)]' : 'text-gray-400'
            } hover:bg-[var(--sidebar-bg)] hover:text-white`}
          title={isDetailVisible ? "Hide Detail" : "Show Detail"}
        >
          {isDetailVisible ? <VscLayoutSidebarRight size={14} /> : <VscLayoutSidebarRightOff size={14} />}
        </button>

        {/* Settings Button */}
        <button
          onClick={() => openSettings()}
          className="w-10 h-10 flex items-center justify-center text-gray-400 hover:bg-[var(--sidebar-bg)] hover:text-white transition-colors"
          title="Settings"
        >
          <VscSettingsGear size={14} />
        </button>
        {/* Window Controls */}
        <div className="flex items-center h-full">
          <button
            onClick={() => getCurrentWindow().minimize()}
            className="w-10 h-10 flex items-center justify-center text-gray-400 hover:bg-[var(--sidebar-bg)] hover:text-white transition-colors"
          >
            <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M20 12H4" />
            </svg>
          </button>
          <button
            onClick={() => getCurrentWindow().toggleMaximize()}
            className="w-10 h-10 flex items-center justify-center text-gray-400 hover:bg-[var(--sidebar-bg)] hover:text-white transition-colors"
          >
            <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <rect x="4" y="4" width="16" height="16" strokeWidth={2} />
            </svg>
          </button>
          <button
            onClick={() => getCurrentWindow().close()}
            className="w-12 h-10 flex items-center justify-center text-gray-400 hover:bg-red-600 hover:text-white transition-colors"
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
      </div>
    </div>
  );
}
