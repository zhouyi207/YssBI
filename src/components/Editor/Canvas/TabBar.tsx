import React, { useState, useRef, useEffect } from "react";
import { useCanvas } from "../Context/CanvasContext";

export const TabBar: React.FC = () => {
  const { tabs, activeTabId, setActiveTabId, addEvent, addFunction, addMacro, closeTab, executeGraph } = useCanvas();
  const [isAddMenuOpen, setIsAddMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!isAddMenuOpen) return;
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setIsAddMenuOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [isAddMenuOpen]);

  return (
    <div className="flex items-center bg-gray-900 border-b border-gray-800 h-9 select-none">
      <div className="flex-1 flex items-center overflow-x-auto no-scrollbar px-2 h-full">
        {tabs.map((tab) => (
          <div
            key={tab.id}
            onClick={() => setActiveTabId(tab.id)}
            className={`
              flex items-center gap-2 px-3 h-full border-r border-gray-800 cursor-pointer transition-colors shrink-0
              ${activeTabId === tab.id ? "bg-gray-800 text-blue-400" : "text-gray-400 hover:bg-gray-800/50"}
            `}
          >
            <span className="text-xs truncate max-w-[120px]">
              {tab.title}{tab.isDirty ? "*" : ""}
            </span>
            <button
              onClick={(e) => closeTab(tab.id, e)}
              className="p-0.5 rounded-full hover:bg-gray-700 text-gray-500 hover:text-white transition-colors"
            >
              <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>
        ))}
      </div>
      <div className="relative shrink-0 border-r border-gray-800 h-full flex items-center">
        <button
          onClick={() => setIsAddMenuOpen(!isAddMenuOpen)}
          className={`px-2 h-full transition-colors shrink-0 ${isAddMenuOpen ? "text-blue-400 bg-gray-800" : "text-gray-500 hover:text-white hover:bg-gray-800"}`}
          title="New Item"
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
          </svg>
        </button>


        {isAddMenuOpen && (
          <div
            ref={menuRef}
            className="absolute top-full left-0 mt-0 w-36 bg-gray-800 border border-gray-700 rounded-b shadow-2xl z-[100] py-1"
          >
            <button
              onClick={() => { addEvent("New Event"); setIsAddMenuOpen(false); }}
              className="w-full text-left px-3 py-2 text-[11px] text-gray-300 hover:bg-blue-600 hover:text-white transition-colors flex items-center gap-2 group"
            >
              <div className="w-1.5 h-1.5 rounded-full bg-red-500 shadow-[0_0_5px_rgba(239,68,68,0.5)]" />
              <span>Event Graph</span>
            </button>
            <button
              onClick={() => { addFunction("New Function"); setIsAddMenuOpen(false); }}
              className="w-full text-left px-3 py-2 text-[11px] text-gray-300 hover:bg-blue-600 hover:text-white transition-colors flex items-center gap-2 group"
            >
              <div className="w-1.5 h-1.5 rounded-full bg-blue-500 shadow-[0_0_5px_rgba(59,130,246,0.5)]" />
              <span>Function</span>
            </button>
            <button
              onClick={() => { addMacro("New Macro"); setIsAddMenuOpen(false); }}
              className="w-full text-left px-3 py-2 text-[11px] text-gray-300 hover:bg-blue-600 hover:text-white transition-colors flex items-center gap-2 group"
            >
              <div className="w-1.5 h-1.5 rounded-full bg-purple-500 shadow-[0_0_5px_rgba(168,85,247,0.5)]" />
              <span>Macro</span>
            </button>
          </div>
        )}

      </div>

      {/* Fixed Execute Button on the right */}
      <div className="flex items-center px-3 border-l border-gray-800 h-full bg-gray-900 shadow-[-10px_0_15px_-5px_rgba(0,0,0,0.5)]">
        <button
          onClick={() => executeGraph()}
          disabled={!activeTabId}
          className={`
            flex items-center gap-1 px-3 py-1 rounded transition-all active:scale-95 text-[10px] font-bold
            ${activeTabId ? "bg-green-600 hover:bg-green-500 text-white" : "bg-gray-800 text-gray-600 cursor-not-allowed"}
          `}        >
          <svg className="w-3 h-3" fill="currentColor" viewBox="0 0 24 24">
            <path d="M8 5v14l11-7z" />
          </svg>
          执行
        </button>
      </div>
    </div>
  );
};
