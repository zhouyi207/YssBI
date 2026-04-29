import React, { useRef, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { VscSplitHorizontal, VscSplitVertical, VscChromeClose } from "react-icons/vsc";
import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { OverlayScrollbar } from "@/shared/ui/OverlayScrollbar";
import { LayoutTab } from "@/shared/types/ui";
import { useDraggable, useDroppable } from "@dnd-kit/core";
import { useShallow } from "zustand/react/shallow";
import { Button } from "@/components/ui/button";
import { addGlobalEventListener } from "@/shared/utils/globalEvent";

interface TabBarProps {
    layoutNodeId: string;
    tabs: LayoutTab[];
    activeTabId?: string;
}

export const TabBar: React.FC<TabBarProps> = ({ layoutNodeId, tabs = [], activeTabId }) => {
  const { t } = useTranslation();
  // 使用 useShallow 和单个选择器订阅所有需要的状态，避免多次重渲染
  const { 
    updateNode, 
    splitNode, 
    removeNode, 
    setActiveGroup, 
    isAltPressed, 
    isDragging 
  } = useLayoutStore(useShallow(s => ({
    updateNode: s.updateNode,
    splitNode: s.splitNode,
    removeNode: s.removeNode,
    setActiveGroup: s.setActiveGroup,
    isAltPressed: s.isAltPressed,
    isDragging: s.isDragging,
  })));

  const containerRef = useRef<HTMLDivElement>(null);
  const [dropIndicatorIndex, setDropIndicatorIndex] = React.useState<number | null>(null);
  
  // 为 TabBar 添加 droppable 区域，用于标签页移动（作为最后位置的 fallback）
  const { setNodeRef: setDropRef, isOver: isTabBarOver } = useDroppable({
    id: `tabbar-${layoutNodeId}`,
    data: { dropType: 'tabbar', targetNodeId: layoutNodeId, targetTabIndex: tabs.length }
  });
  
  // 当拖动到 TabBar 的空白区域时，显示在最后位置的指示器
  React.useEffect(() => {
    if (isTabBarOver && isDragging) {
      setDropIndicatorIndex(tabs.length);
    }
  }, [isTabBarOver, isDragging, tabs.length]);

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
    
    // 优先使用事件中的 altKey 状态，实现零延迟响应
    const currentAlt = e.altKey || isAltPressed;
    
    // 默认左右分栏 (row)，按住 Alt 时上下分栏 (col)
    // 注意：在我们的布局引擎中，'row' 表示子节点水平排列（左右），'col' 表示垂直排列（上下）
    const direction = currentAlt ? 'col' : 'row';
    
    splitNode(layoutNodeId, direction, activeTab?.component || 'GraphEditor');
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
  
  // 拖动结束时清除插入指示器
  useEffect(() => {
    if (!isDragging) {
      setDropIndicatorIndex(null);
    }
  }, [isDragging]);
  
  // 在拖动时禁用滚动
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    
    if (isDragging) {
      const preventScroll = (e: WheelEvent) => {
        e.preventDefault();
        e.stopPropagation();
      };
      
      container.addEventListener('wheel', preventScroll, { passive: false });
      return () => {
        container.removeEventListener('wheel', preventScroll);
      };
    }
  }, [isDragging]);

  return (
    <div 
      ref={setDropRef}
      className="flex items-center border-b w-full shrink-0 select-none overflow-hidden bg-[var(--workbench-bg)] border-[var(--strong-border)]"
      style={{ height: 'var(--titlebar-height)' }}
    >
      <div className="relative flex-1 flex items-start h-full min-w-0">
        {isDragging ? (
          <div ref={containerRef} className="absolute inset-0 overflow-hidden flex items-start">
            {tabs.map((tab, index) => (
              <TabItem
                key={tab.id}
                tab={tab}
                index={index}
                layoutNodeId={layoutNodeId}
                isActive={activeTabId === tab.id}
                onClick={() => handleTabClick(tab.id)}
                onClose={(e) => handleCloseTab(tab.id, e)}
                onDragOver={(index) => setDropIndicatorIndex(index)}
              />
            ))}
            {dropIndicatorIndex !== null && (
              <div
                className="absolute top-0 bottom-0 w-0.5 bg-[var(--accent-color)] z-50 pointer-events-none"
                style={{
                  left: (() => {
                    const container = containerRef.current;
                    if (!container) return 0;
                    const tabElement = container.children[dropIndicatorIndex] as HTMLElement;
                    if (!tabElement) {
                      const lastTab = container.children[tabs.length - 1] as HTMLElement;
                      if (lastTab) return lastTab.offsetLeft + lastTab.offsetWidth;
                      return 0;
                    }
                    return tabElement.offsetLeft;
                  })(),
                }}
              />
            )}
          </div>
        ) : (
          <OverlayScrollbar ref={containerRef} direction="horizontal" className="flex-1 flex items-start h-full">
            {tabs.map((tab, index) => (
              <TabItem
                key={tab.id}
                tab={tab}
                index={index}
                layoutNodeId={layoutNodeId}
                isActive={activeTabId === tab.id}
                onClick={() => handleTabClick(tab.id)}
                onClose={(e) => handleCloseTab(tab.id, e)}
                onDragOver={(index) => setDropIndicatorIndex(index)}
              />
            ))}
          </OverlayScrollbar>
        )}
      </div>

      {/* Group Action Buttons */}
      <div className="flex items-center gap-0.5 px-1 border-l border-[var(--strong-border)] h-full bg-[var(--workbench-bg)]">
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          onPointerDown={(e) => {
            // 使用 PointerDown 代替 Click，消除点击抬起的延迟感
            if (e.button !== 0) return;
            handleSplit(e as any);
          }}
          onMouseEnter={(e) => {
            // 鼠标移入时同步 Alt 状态，确保图标及时更新
            if (e.altKey !== isAltPressed) {
                useLayoutStore.getState().setAltPressed(e.altKey);
            }
          }}
          className="text-muted-foreground"
          title={isAltPressed ? t("tabBar.splitDownAlt") : t("tabBar.splitRight")}
        >
          {isAltPressed ? <VscSplitVertical size={15} /> : <VscSplitHorizontal size={15} />}
        </Button>

        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          onClick={handleCloseGroup}
          className="text-muted-foreground hover:text-red-400"
          title={t("tabBar.closeGroup")}
        >
          <VscChromeClose size={15} />
        </Button>
      </div>
    </div>
  );
};

interface TabItemProps {
    tab: LayoutTab;
    index: number;
    layoutNodeId: string;
    isActive: boolean;
    onClick: () => void;
    onClose: (e: React.MouseEvent) => void;
    onDragOver: (index: number) => void;
}

const TabItem: React.FC<TabItemProps> = React.memo(({ tab, index, layoutNodeId, isActive, onClick, onClose, onDragOver }) => {
    const tabRef = React.useRef<HTMLDivElement>(null);
    
    const { attributes, listeners, setNodeRef, transform, isDragging } = useDraggable({
        id: `tab-${layoutNodeId}-${tab.id}`,
        data: { type: 'tab', tabId: tab.id, sourceNodeId: layoutNodeId }
    });
    
    const { setNodeRef: setDropRef, isOver } = useDroppable({
        id: `tab-drop-${layoutNodeId}-${index}`,
        data: { dropType: 'tabbar', targetNodeId: layoutNodeId, targetTabIndex: index }
    });
    
    // 合并 draggable 和 droppable 的 refs
    const setRefs = React.useCallback((node: HTMLDivElement | null) => {
        tabRef.current = node;
        setNodeRef(node);
        setDropRef(node);
    }, [setNodeRef, setDropRef]);
    
    // 当拖动到这个标签上时，根据鼠标位置决定插入到左边还是右边
    React.useEffect(() => {
        if (!isOver || !tabRef.current) return;
        
        const handleMouseMove = (e: MouseEvent) => {
            const rect = tabRef.current!.getBoundingClientRect();
            const mouseX = e.clientX;
            const tabCenter = rect.left + rect.width / 2;
            
            // 如果鼠标在标签左半部分，插入到当前位置；右半部分则插入到下一个位置
            if (mouseX < tabCenter) {
                onDragOver(index);
            } else {
                onDragOver(index + 1);
            }
        };
        
        return addGlobalEventListener(window, 'mousemove', handleMouseMove);
    }, [isOver, index, onDragOver]);

    const style = transform ? {
        transform: `translate3d(${transform.x}px, ${transform.y}px, 0)`,
        zIndex: 100,
        opacity: isDragging ? 0.5 : 1,
        height: 'var(--titlebar-height)',
    } : {
        opacity: isDragging ? 0.5 : 1,
        height: 'var(--titlebar-height)',
    };

    return (
        <div
            ref={setRefs}
            style={style}
            {...attributes}
            {...listeners}
            data-tab-id={tab.id}
            onClick={onClick}
            className={`
                relative flex items-center gap-2 px-3 border-r border-[var(--strong-border)] cursor-pointer shrink-0
                ${isActive ? "bg-[var(--sidebar-bg)] text-foreground" : "text-muted-foreground hover:bg-muted"}
                ${isDragging ? 'cursor-grabbing' : 'cursor-pointer'}
            `}
        >
            {/* Active Top Border */}
            {isActive && (
                <div className="absolute top-0 left-0 right-0 h-[2px] bg-[var(--accent-color)]" />
            )}
            
            <span className={`text-xs truncate max-w-[120px]`}>
                {tab.title}
            </span>
            <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                onClick={onClose}
                className="text-muted-foreground hover:text-foreground"
            >
                <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2.5} d="M6 18L18 6M6 6l12 12" />
                </svg>
            </Button>
        </div>
    );
});
