import React, { useRef, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { VscSplitHorizontal, VscSplitVertical, VscChromeClose } from "react-icons/vsc";
import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { getActiveLayoutTab } from "@/features/core/layout/layoutTabQueries";
import { splitComponentForTab } from "@/features/core/layout/layoutTabModel";
import { OverlayScrollbar } from "@/shared/ui/OverlayScrollbar";
import { LayoutTab } from "@/shared/types/ui";
import { useDraggable, useDroppable } from "@dnd-kit/core";
import { useShallow } from "zustand/react/shallow";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import {
  editorTabBarActionsClass,
  editorTabBarShellClass,
  editorTabDropIndicatorClass,
  editorTabItemVariants,
} from "./editorTabStyles";
import { addGlobalEventListener } from "@/shared/utils/globalEvent";
import { DROP_TYPES, DRAG_TYPES } from "@/features/core/dnd";
import { releaseGraphCacheIfClosed } from "@/features/application/editor/releaseGraphCache";
import { closeEditorTab } from "@/features/application/editor/closeEditorTab";
import { switchEditorGraphTab } from "@/features/application/editor/switchEditorGraphTab";
import { resourceKey, resourceRefFromLayoutTab, useDocumentStateStore, useResourceStore } from "@/features/core/resource";

interface TabBarProps {
    layoutNodeId: string;
    tabs: LayoutTab[];
    activeTabId?: string;
}

export const TabBar: React.FC<TabBarProps> = ({ layoutNodeId, tabs = [], activeTabId }) => {
  const { t } = useTranslation();
  // 使用 useShallow 和单个选择器订阅所有需要的状态，避免多次重渲染
  const { 
    splitNode, 
    removeNode, 
    isAltPressed, 
    isDragging 
  } = useLayoutStore(useShallow(s => ({
    splitNode: s.splitNode,
    removeNode: s.removeNode,
    isAltPressed: s.isAltPressed,
    isDragging: s.isDragging,
  })));

  const containerRef = useRef<HTMLDivElement>(null);
  const [dropIndicatorIndex, setDropIndicatorIndex] = React.useState<number | null>(null);
  
  // 为 TabBar 添加 droppable 区域，用于标签页移动（作为最后位置的 fallback）
  const { setNodeRef: setDropRef, isOver: isTabBarOver } = useDroppable({
    id: `tabbar-${layoutNodeId}`,
    data: { dropType: DROP_TYPES.TABBAR, targetNodeId: layoutNodeId, targetTabIndex: tabs.length }
  });
  
  // 当拖动到 TabBar 的空白区域时，显示在最后位置的指示器
  React.useEffect(() => {
    if (isTabBarOver && isDragging) {
      setDropIndicatorIndex(tabs.length);
    }
  }, [isTabBarOver, isDragging, tabs.length]);

  const handleTabClick = (id: string) => {
    const currentData = useLayoutStore.getState().nodes[layoutNodeId].data;
    const tab = currentData?.tabs?.find((item) => item.id === id);
    void switchEditorGraphTab(layoutNodeId, id, tab);
  };

  const handleCloseTab = (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    void closeEditorTab(id, layoutNodeId);
  };

  const handleSplit = (e: Pick<PointerEvent, 'altKey' | 'stopPropagation'>) => {
    e.stopPropagation();
    const nodes = useLayoutStore.getState().nodes;
    const activeTab = getActiveLayoutTab(layoutNodeId, nodes)?.tab;

    const currentAlt = e.altKey || isAltPressed;
    const direction = currentAlt ? 'col' : 'row';

    splitNode(layoutNodeId, direction, splitComponentForTab(activeTab));
  };

  const handleCloseGroup = async (e: React.MouseEvent) => {
    e.stopPropagation();
    const tabIds = useLayoutStore.getState().nodes[layoutNodeId]?.data?.tabs?.map((tab) => tab.id) ?? [];
    for (const tabId of tabIds) {
      const closed = await closeEditorTab(tabId, layoutNodeId);
      if (!closed) return;
    }
    removeNode(layoutNodeId);
    tabIds.forEach(releaseGraphCacheIfClosed);
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

  // 拖动时在 TabBar 容器上拦截滚轮，避免误滚动（组件级 listener，非全局）
  useEffect(() => {
    const container = containerRef.current;
    if (!container || !isDragging) return;

    const preventScroll = (e: WheelEvent) => {
      e.preventDefault();
      e.stopPropagation();
    };

    container.addEventListener('wheel', preventScroll, { passive: false });
    return () => {
      container.removeEventListener('wheel', preventScroll);
    };
  }, [isDragging]);

  return (
    <div
      ref={setDropRef}
      className={editorTabBarShellClass}
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
                className={editorTabDropIndicatorClass}
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
      <div className={editorTabBarActionsClass}>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              onPointerDown={(e) => {
                if (e.button !== 0) return;
                handleSplit(e);
              }}
              onMouseEnter={(e) => {
                if (e.altKey !== isAltPressed) {
                  useLayoutStore.getState().setAltPressed(e.altKey);
                }
              }}
              className="text-muted-foreground"
            >
              {isAltPressed ? <VscSplitVertical size={15} /> : <VscSplitHorizontal size={15} />}
            </Button>
          </TooltipTrigger>
          <TooltipContent side="bottom">
            {isAltPressed ? t("tabBar.splitDownAlt") : t("tabBar.splitRight")}
          </TooltipContent>
        </Tooltip>

        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              onClick={handleCloseGroup}
              className="text-muted-foreground hover:text-red-400"
            >
              <VscChromeClose size={15} />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="bottom">{t("tabBar.closeGroup")}</TooltipContent>
        </Tooltip>
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
    const resourceRef = resourceRefFromLayoutTab(tab);
    const resourceTitle = useResourceStore((state) => {
        if (!resourceRef) return undefined;
        return state.resources[resourceKey(resourceRef)]?.name;
    });
    const documentState = useDocumentStateStore((state) => {
        if (!resourceRef) return undefined;
        return state.documents[resourceKey(resourceRef)];
    });
    const title = resourceTitle ?? tab.title;
    const statusLabel = documentState?.missing
        ? "missing"
        : documentState?.conflict
            ? "conflict"
            : documentState?.stale
                ? "stale"
                : null;
    const isDirty = resourceRef ? (documentState?.dirty ?? false) : false;
    
    const { attributes, listeners, setNodeRef, transform, isDragging } = useDraggable({
        id: `tab-${layoutNodeId}-${tab.id}`,
        data: { type: DRAG_TYPES.TAB, tabId: tab.id, sourceNodeId: layoutNodeId }
    });
    
    const { setNodeRef: setDropRef, isOver } = useDroppable({
        id: `tab-drop-${layoutNodeId}-${index}`,
        data: { dropType: DROP_TYPES.TABBAR, targetNodeId: layoutNodeId, targetTabIndex: index }
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
            className={editorTabItemVariants({ active: isActive, dragging: isDragging })}
        >
            <span className="max-w-[120px] truncate">
                {title}
            </span>
            {statusLabel ? (
                <span className="ml-1 text-[10px] uppercase text-amber-500">
                    {statusLabel}
                </span>
            ) : null}
            <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                onClick={onClose}
                className="text-muted-foreground hover:text-foreground"
            >
                {isDirty ? (
                    <span className="h-2 w-2 rounded-full bg-current" />
                ) : (
                    <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2.5} d="M6 18L18 6M6 6l12 12" />
                    </svg>
                )}
            </Button>
        </div>
    );
});
