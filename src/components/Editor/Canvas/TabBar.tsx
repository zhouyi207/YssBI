import React, { useState, useRef, useEffect, useContext } from "react";
import { useCanvas, GroupContext } from "../Context/CanvasContext";
import { VscSplitHorizontal, VscChromeClose } from "react-icons/vsc";
export const TabBar: React.FC = () => {
  const { tabs, activeTabId, setActiveTabId, closeTab, splitEditorRight, groups, closeGroup } = useCanvas();
  const currentGroupId = useContext(GroupContext);
  const containerRef = useRef<HTMLDivElement>(null);
  const [indicatorStyle, setIndicatorStyle] = useState({ left: 0, width: 0, opacity: 0 });

  // Update sliding indicator position
  useEffect(() => {
    const container = containerRef.current;
    if (!container || !activeTabId) {
      setIndicatorStyle(prev => ({ ...prev, opacity: 0 }));
      return;
    }

    const activeEl = container.querySelector(`[data-tab-id="${activeTabId}"]`) as HTMLElement;
    if (activeEl) {
      setIndicatorStyle({
        left: activeEl.offsetLeft,
        width: activeEl.offsetWidth,
        opacity: 1
      });

      // Auto-scroll to make the active tab visible
      activeEl.scrollIntoView({
        behavior: 'smooth',
        block: 'nearest',
        inline: 'nearest'
      });
    } else {
      setIndicatorStyle(prev => ({ ...prev, opacity: 0 }));
    }
  }, [activeTabId, tabs]);

  if (tabs.length === 0) return null;

  return (
    <div className="flex items-center bg-[var(--workbench-bg)] border-b border-[#2b2b2b] h-9 shrink-0 select-none">
      <div ref={containerRef} className="relative flex-1 flex items-start overflow-x-auto tab-scrollbar h-full">
        {/* Sliding Indicator (Back inside to move WITH content) */}
        <div
          className="absolute top-0 h-[1px] bg-[var(--accent-color)] transition-all duration-300 ease-in-out z-20 pointer-events-none"
          style={{
            left: 0,
            width: indicatorStyle.width,
            transform: `translateX(${indicatorStyle.left}px)`,
            opacity: indicatorStyle.opacity
          }}
        />

        {tabs.map((tab) => (
          <div
            key={tab.id}
            data-tab-id={tab.id}
            onClick={() => setActiveTabId(tab.id)}
            className={`
              relative flex items-center gap-2 px-3 h-9 border-r border-[#2b2b2b] cursor-pointer transition-colors shrink-0
              ${activeTabId === tab.id ? "bg-[var(--sidebar-bg)]" : "text-gray-500 hover:bg-white/5"}
            `}
          >
            <span className={`text-xs truncate max-w-[120px] transition-colors duration-200 ${activeTabId === tab.id ? "text-white" : ""}`}>
              {tab.title}{tab.isDirty ? "*" : ""}
            </span>
            <button
              onClick={(e) => closeTab(tab.id, e)}
              className="p-0.5 rounded-sm hover:bg-white/10 text-gray-500 hover:text-white transition-colors"
            >
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2.5} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>
        ))}
      </div>


      {/* Group Action Buttons */}
      <div className="flex items-center gap-0.5 px-1 border-l border-[#2b2b2b] h-full">
        <button
          onClick={() => currentGroupId && splitEditorRight(currentGroupId)}
          className="p-1 px-1.5 text-gray-500 hover:text-white hover:bg-white/5 transition-colors rounded-sm"
          title="Split Editor Right"
        >
          <VscSplitHorizontal size={15} />
        </button>

        {groups.length > 1 && (
          <button
            onClick={() => currentGroupId && closeGroup(currentGroupId)}
            className="p-1 px-1.5 text-gray-500 hover:text-red-400 hover:bg-white/5 transition-colors rounded-sm"
            title="Close Group"
          >
            <VscChromeClose size={15} />
          </button>
        )}
      </div>

    </div >
  );
};
