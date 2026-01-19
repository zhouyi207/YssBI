import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useCanvas } from "./canvas/CanvasContext";
import { useState } from "react";

const appWindow = getCurrentWindow();

interface MenuItem {
  label: string;
  onClick?: () => void;
}

const MenuButton = ({ label, items }: { label: string; items: MenuItem[] }) => {
  const [isOpen, setIsOpen] = useState(false);

  return (
    <div className="relative group">
      <button
        onMouseEnter={() => setIsOpen(true)}
        onMouseLeave={() => setIsOpen(false)}
        className="px-3 py-1 text-sm text-gray-400 hover:text-white hover:bg-gray-800 rounded transition-colors"
      >
        {label}
      </button>
      {isOpen && (
        <div 
          onMouseEnter={() => setIsOpen(true)}
          onMouseLeave={() => setIsOpen(false)}
          className="absolute left-0 top-full w-44 bg-gray-800 border border-gray-700 rounded shadow-xl py-1 z-50"
        >
          {items.map((item, i) => (
            <div
              key={i}
              onClick={() => {
                if (item.onClick) {
                  item.onClick();
                  setIsOpen(false);
                }
              }}
              className={`px-4 py-1.5 text-xs flex justify-between items-center transition-colors ${
                item.onClick 
                  ? "text-gray-300 hover:bg-blue-600 hover:text-white cursor-pointer" 
                  : "text-gray-600 cursor-default"
              }`}
            >
              <span>{item.label}</span>
              {!item.onClick && <span className="text-[9px] opacity-40 ml-2">⏳</span>}
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

export default function Menubar() {
  const { exportGraph, importGraph, executeGraph, saveGraph } = useCanvas();

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
    { label: "Open...", onClick: () => importGraph() },
    { label: "Save", onClick: () => saveGraph() },
    { label: "Export As...", onClick: () => exportGraph() },
  ];

  const editItems: MenuItem[] = [
    { label: "Undo" },
    { label: "Redo" },
    { label: "Copy" },
    { label: "Paste" },
    { label: "Delete" },
  ];

  const dataItems: MenuItem[] = [
    { label: "Manage Variables" },
    { label: "Import Data" },
    { label: "Schema Viewer" },
  ];

  const windowItems: MenuItem[] = [
    { label: "New Window", onClick: openNewWindow },
    { label: "Reset Layout" },
    { label: "Zoom In" },
    { label: "Zoom Out" },
  ];

  const toolItems: MenuItem[] = [
    { label: "Debugger" },
    { label: "Profiler" },
    { label: "Settings" },
  ];

  const helpItems: MenuItem[] = [
    { label: "Documentation" },
    { label: "Release Notes" },
    { label: "About" },
  ];

  return (
    <div 
      className="menubar-container h-10 bg-gray-900 border-b border-gray-800 flex items-center z-50 shadow-xl select-none"
      onWheel={(e) => e.stopPropagation()}
      data-tauri-drag-region
    >
      {/* Left: Icon & Brand */}
      <div className="flex items-center gap-2 px-4 pointer-events-none">
        <div className="w-5 h-5 bg-blue-600 rounded flex items-center justify-center">
          <span className="text-white font-black text-xs">Y</span>
        </div>
        <div className="text-white font-bold text-sm tracking-tight">
          Yss<span className="text-blue-500">BI</span>
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

      {/* Right side: Controls & Window Buttons */}
      <div className="flex items-center h-full">
        {/* Execution Button (Small version) */}
        <button
          onClick={() => executeGraph()}
          className="flex items-center gap-1 px-3 py-1 mr-4 rounded bg-green-600 hover:bg-green-500 text-white transition-all active:scale-95 text-xs font-bold"
        >
          <svg className="w-3 h-3" fill="currentColor" viewBox="0 0 24 24">
            <path d="M8 5v14l11-7z" />
          </svg>
          执行
        </button>

        {/* Window Controls */}
        <div className="flex items-center h-full">
          <button
            onClick={() => appWindow.minimize()}
            className="w-10 h-10 flex items-center justify-center text-gray-400 hover:bg-gray-800 hover:text-white transition-colors"
          >
            <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M20 12H4" />
            </svg>
          </button>
          <button
            onClick={() => appWindow.toggleMaximize()}
            className="w-10 h-10 flex items-center justify-center text-gray-400 hover:bg-gray-800 hover:text-white transition-colors"
          >
            <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <rect x="4" y="4" width="16" height="16" strokeWidth={2} />
            </svg>
          </button>
          <button
            onClick={() => appWindow.close()}
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
