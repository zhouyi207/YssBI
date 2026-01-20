import React from "react";
import { useCanvas } from "./CanvasContext";

export const TabBar: React.FC = () => {
  const { tabs, activeTabId, setActiveTabId, addTab, closeTab, executeGraph } = useCanvas();

  return (
    <div className="flex items-center bg-gray-900 border-b border-gray-800 h-9 select-none overflow-hidden">
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
            <div className="flex items-center gap-1 min-w-0 max-w-[120px]">
              <span className="text-xs truncate flex-1">{tab.title}</span>
              {tab.isDirty && <div className="w-1.5 h-1.5 rounded-full bg-blue-400 shrink-0" />}
            </div>
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
        <button
          onClick={() => addTab()}
          className="p-2 text-gray-500 hover:text-white hover:bg-gray-800 transition-colors shrink-0"
          title="New Tab"
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
          </svg>
        </button>
      </div>

      {/* Fixed Execute Button on the right */}
      <div className="flex items-center px-3 border-l border-gray-800 h-full bg-gray-900 shadow-[-10px_0_15px_-5px_rgba(0,0,0,0.5)]">
        <button
          onClick={() => executeGraph()}
          className="flex items-center gap-1 px-3 py-1 rounded bg-green-600 hover:bg-green-500 text-white transition-all active:scale-95 text-[10px] font-bold"
        >
          <svg className="w-3 h-3" fill="currentColor" viewBox="0 0 24 24">
            <path d="M8 5v14l11-7z" />
          </svg>
          执行
        </button>
      </div>
    </div>
  );
};
