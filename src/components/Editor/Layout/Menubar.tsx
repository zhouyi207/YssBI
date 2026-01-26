import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCanvas } from "../Context/CanvasContext";
import { useState } from "react";
import { useLayoutStore } from "../../../store/layoutStore";
import { VscLayoutSidebarRight, VscLayoutSidebarRightOff, VscSettingsGear } from "react-icons/vsc";

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

export default function Menubar() {
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
  } = useCanvas();

  const openSettings = useLayoutStore(s => s.openSettings);
  const activeEditorGroupId = useLayoutStore(s => s.activeEditorGroupId);
  const splitNode = useLayoutStore(s => s.splitNode);

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
    { label: "Import Data" },
    { label: "-" },
    { label: "Schema Viewer" },
  ];

  const windowItems: MenuItem[] = [
    { label: "New Window", onClick: openNewWindow },
    { label: "-" },
    { label: "Split Editor Right", onClick: handleSplitRight },
    { label: "Split Editor Down", onClick: handleSplitDown },
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
