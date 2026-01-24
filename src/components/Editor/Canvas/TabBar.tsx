import React, { useRef, useEffect } from "react";
import { VscSplitHorizontal, VscChromeClose } from "react-icons/vsc";
import { useLayoutStore } from "../../../store/layoutStore";
import { LayoutTab } from "../../../types/layout";
import { useDraggable } from "@dnd-kit/core";

interface TabBarProps {
    layoutNodeId: string;
    tabs: LayoutTab[];
    activeTabId?: string;
}

export const TabBar: React.FC<TabBarProps> = ({ layoutNodeId, tabs = [], activeTabId }) => {
  const updateNode = useLayoutStore(s => s.updateNode);
  const splitNode = useLayoutStore(s => s.splitNode);
  const removeNode = useLayoutStore(s => s.removeNode);
  const setActiveGroup = useLayoutStore(s => s.setActiveGroup);
  const isActiveGroup = useLayoutStore(s => s.activeGroupId === layoutNodeId);

  const containerRef = useRef<HTMLDivElement>(null);

  const handleTabClick = (id: string) => {
    setActiveGroup(layoutNodeId);
    updateNode(layoutNodeId, {
        data: {
            ...useLayoutStore.getState().nodes[layoutNodeId].data,
            activeTabId: id
        }
    });
  };

  const handleCloseTab = (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    useLayoutStore.getState().removeTab(layoutNodeId, id);
  };

  const handleSplit = (e: React.MouseEvent) => {
    e.stopPropagation();
    const node = useLayoutStore.getState().nodes[layoutNodeId];
    const activeTab = node.data?.tabs?.find(t => t.id === node.data?.activeTabId);
    splitNode(layoutNodeId, 'row', activeTab?.component || 'GraphEditor');
  };

  const handleCloseGroup = (e: React.MouseEvent) => {
    e.stopPropagation();
    removeNode(layoutNodeId);
  };

  // Auto-scroll to active tab
  useEffect(() => {
    const container = containerRef.current;
    if (!container || !activeTabId) return;

    const activeEl = container.querySelector(`[data-tab-id="${activeTabId}"]`) as HTMLElement;
    if (activeEl) {
      activeEl.scrollIntoView({
        behavior: 'smooth',
        block: 'nearest',
        inline: 'nearest'
      });
    }
  }, [activeTabId]);

  return (
    <div className={`flex items-center border-b h-9 w-full shrink-0 select-none overflow-hidden transition-colors ${isActiveGroup ? 'bg-[var(--workbench-bg)] border-[#3e3e3e]' : 'bg-[#252526] border-transparent'}`}>
      <div ref={containerRef} className="relative flex-1 flex items-start overflow-x-auto tab-scrollbar h-full no-scrollbar">
        {tabs.map((tab, index) => (
          <TabItem
            key={tab.id}
            tab={tab}
            index={index}
            layoutNodeId={layoutNodeId}
            isActive={activeTabId === tab.id}
            isActiveGroup={isActiveGroup}
            onClick={() => handleTabClick(tab.id)}
            onClose={(e) => handleCloseTab(tab.id, e)}
          />
        ))}
      </div>

      {/* Group Action Buttons */}
      <div className={`flex items-center gap-0.5 px-1 border-l border-[#2b2b2b] h-full transition-colors ${isActiveGroup ? 'bg-[var(--workbench-bg)]' : 'bg-[#252526]'}`}>
        <button
          onClick={handleSplit}
          className="p-1 px-1.5 text-gray-500 hover:text-white hover:bg-white/5 transition-colors rounded-sm"
          title="Split Editor Right"
        >
          <VscSplitHorizontal size={15} />
        </button>

        <button
          onClick={handleCloseGroup}
          className="p-1 px-1.5 text-gray-500 hover:text-red-400 hover:bg-white/5 transition-colors rounded-sm"
          title="Close Group"
        >
          <VscChromeClose size={15} />
        </button>
      </div>
    </div>
  );
};

interface TabItemProps {
    tab: LayoutTab;
    index: number;
    layoutNodeId: string;
    isActive: boolean;
    isActiveGroup: boolean;
    onClick: () => void;
    onClose: (e: React.MouseEvent) => void;
}

const TabItem: React.FC<TabItemProps> = ({ tab, layoutNodeId, isActive, isActiveGroup, onClick, onClose }) => {
    const { attributes, listeners, setNodeRef, transform, isDragging } = useDraggable({
        id: `tab-${layoutNodeId}-${tab.id}`,
        data: { type: 'tab', tabId: tab.id, sourceNodeId: layoutNodeId }
    });

    const style = transform ? {
        transform: `translate3d(${transform.x}px, ${transform.y}px, 0)`,
        zIndex: 100,
        opacity: isDragging ? 0.5 : 1
    } : {
        opacity: isDragging ? 0.5 : 1
    };

    return (
        <div
            ref={setNodeRef}
            style={style}
            {...attributes}
            {...listeners}
            data-tab-id={tab.id}
            onClick={onClick}
            className={`
                relative flex items-center gap-2 px-3 h-9 border-r border-[#2b2b2b] cursor-pointer transition-colors shrink-0
                ${isActive 
                    ? (isActiveGroup ? "bg-[var(--sidebar-bg)] text-white" : "bg-[var(--sidebar-bg)]/60 text-gray-300") 
                    : "text-gray-500 hover:bg-white/5"}
                ${isDragging ? 'cursor-grabbing' : 'cursor-pointer'}
            `}
        >
            <span className={`text-xs truncate max-w-[120px] transition-colors duration-200`}>
                {tab.title}
            </span>
            <button
                onClick={onClose}
                className="p-0.5 rounded-sm hover:bg-white/10 text-gray-500 hover:text-white transition-colors"
            >
                <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2.5} d="M6 18L18 6M6 6l12 12" />
                </svg>
            </button>
            
            {/* Active Bottom Border */}
            {isActive && (
                <div className={`absolute bottom-0 left-0 right-0 h-[1px] transition-colors ${isActiveGroup ? 'bg-[var(--accent-color)]' : 'bg-gray-500'}`} />
            )}
        </div>
    );
};
