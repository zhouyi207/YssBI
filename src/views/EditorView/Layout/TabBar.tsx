import React, { useRef, useEffect, useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import { VscSplitHorizontal, VscSplitVertical, VscChromeClose, VscWarning, VscSync, VscError } from "react-icons/vsc";
import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { layoutTabResourceRef } from "@/features/core/layout/layoutTabModel";
import { OverlayScrollbar } from "@/shared/ui/OverlayScrollbar";
import { LayoutTab } from "@/shared/types/ui";
import { useDraggable, useDroppable } from "@dnd-kit/core";
import { useShallow } from "zustand/react/shallow";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { ContextMenu } from "@/shared/ui/contextMenu";
import type { ContextMenuPosition } from "@/shared/ui/contextMenu";
import {
  editorTabBarActionsClass,
  editorTabBarShellClass,
  editorTabDropIndicatorClass,
  editorTabItemVariants,
} from "./editorTabStyles";
import { addGlobalEventListener } from "@/shared/utils/globalEvent";
import { DROP_TYPES, DRAG_TYPES } from "@/features/core/dnd";
import {
  closeEditorGroup,
  closeTab,
  pinTab,
  splitEditorGroupFromPointer,
  switchTab,
} from "@/features/application/editor/tabCommands";
import { buildTabContextMenuSections } from "@/features/application/editor/tabContextMenu";
import { resolveTabDisplayName } from "@/features/application/editor/resolveTabDisplayName";
import { isPreviewLayoutTab } from "@/features/core/layout/layoutTabModel";
import { useSidebarTab } from "@/features/application/editor/useSidebarTab";
import { resourceKey, useDocumentStateStore, useResourceStore } from "@/features/core/resource";
import { isUntitledGraphPath } from "@/shared/types/domain/graphResourcePath";

interface TabBarProps {
    layoutNodeId: string;
    tabs: LayoutTab[];
    activeTabId?: string;
}

export const TabBar: React.FC<TabBarProps> = ({ layoutNodeId, tabs = [], activeTabId }) => {
  const { t } = useTranslation();
  const switchSidebarTab = useSidebarTab();
  const { isAltPressed, isDragging } = useLayoutStore(useShallow(s => ({
    isAltPressed: s.isAltPressed,
    isDragging: s.isDragging,
  })));

  const containerRef = useRef<HTMLDivElement>(null);
  const [dropIndicatorIndex, setDropIndicatorIndex] = React.useState<number | null>(null);

  const { setNodeRef: setDropRef, isOver: isTabBarOver } = useDroppable({
    id: `tabbar-${layoutNodeId}`,
    data: { dropType: DROP_TYPES.TABBAR, targetNodeId: layoutNodeId, targetTabIndex: tabs.length }
  });

  React.useEffect(() => {
    if (isTabBarOver && isDragging) {
      setDropIndicatorIndex(tabs.length);
    }
  }, [isTabBarOver, isDragging, tabs.length]);

  const handleTabClick = (tab: LayoutTab) => {
    void switchTab(layoutNodeId, tab.id, tab);
  };

  const handleCloseTab = (tabId: string, e: React.MouseEvent) => {
    e.stopPropagation();
    void closeTab(layoutNodeId, tabId);
  };

  const handleSplit = (e: Pick<PointerEvent, 'altKey' | 'stopPropagation'>) => {
    e.stopPropagation();
    splitEditorGroupFromPointer(layoutNodeId, e.altKey || isAltPressed);
  };

  const handleCloseGroup = async (e: React.MouseEvent) => {
    e.stopPropagation();
    await closeEditorGroup(layoutNodeId);
  };

  const revealInSidebar = useCallback((tab: LayoutTab) => {
    if (tab.type === 'event' || tab.type === 'function') {
      switchSidebarTab('graphs');
    } else if (tab.type === 'worksheet') {
      switchSidebarTab('charts');
    }
  }, [switchSidebarTab]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container || !activeTabId) return;
    const activeEl = container.querySelector(`[data-tab-id="${activeTabId}"]`) as HTMLElement;
    if (activeEl) {
      activeEl.scrollIntoView({ behavior: 'smooth', block: 'nearest', inline: 'nearest' });
    }
  }, [activeTabId]);

  useEffect(() => {
    if (!isDragging) setDropIndicatorIndex(null);
  }, [isDragging]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container || !isDragging) return;
    const preventScroll = (e: WheelEvent) => {
      e.preventDefault();
      e.stopPropagation();
    };
    container.addEventListener('wheel', preventScroll, { passive: false });
    return () => container.removeEventListener('wheel', preventScroll);
  }, [isDragging]);

  const tabList = (
    <>
      {tabs.map((tab, index) => (
        <TabItem
          key={tab.id}
          tab={tab}
          index={index}
          layoutNodeId={layoutNodeId}
          isActive={activeTabId === tab.id}
          onClick={() => handleTabClick(tab)}
          onClose={(e) => handleCloseTab(tab.id, e)}
          onDragOver={(idx) => setDropIndicatorIndex(idx)}
          onRevealInSidebar={() => revealInSidebar(tab)}
        />
      ))}
    </>
  );

  return (
    <div ref={setDropRef} className={editorTabBarShellClass}>
      <div className="relative flex-1 flex items-start h-full min-w-0">
        {isDragging ? (
          <div ref={containerRef} className="absolute inset-0 overflow-hidden flex items-start">
            {tabList}
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
            {tabList}
          </OverlayScrollbar>
        )}
      </div>

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
    onRevealInSidebar: () => void;
}

const TabItem: React.FC<TabItemProps> = React.memo(({
  tab, index, layoutNodeId, isActive, onClick, onClose, onDragOver, onRevealInSidebar,
}) => {
    const { t } = useTranslation();
    const tabRef = React.useRef<HTMLDivElement>(null);
    const [menuPosition, setMenuPosition] = useState<ContextMenuPosition | null>(null);
    const resourceRef = layoutTabResourceRef(tab);
    const resourceTitle = useResourceStore((state) => {
        if (!resourceRef) return undefined;
        return state.resources[resourceKey(resourceRef)]?.name;
    });
    const documentState = useDocumentStateStore((state) => {
        if (!resourceRef) return undefined;
        return state.documents[resourceKey(resourceRef)];
    });

    const baseTitle = resolveTabDisplayName(resourceRef, tab.id);
    const title = isUntitledGraphPath(tab.id)
      ? `${t('tabBar.unsavedPrefix')}: ${baseTitle}`
      : (resourceTitle ?? baseTitle);

    const isPreview = isPreviewLayoutTab(tab);

    const statusKey = documentState?.missing
        ? 'missing'
        : documentState?.conflict
            ? 'conflict'
            : documentState?.stale
                ? 'stale'
                : null;

    const statusIcon = statusKey === 'missing'
      ? <VscError size={12} className="text-red-500" />
      : statusKey === 'conflict'
        ? <VscWarning size={12} className="text-amber-500" />
        : statusKey === 'stale'
          ? <VscSync size={12} className="text-amber-500" />
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

    const setRefs = React.useCallback((node: HTMLDivElement | null) => {
        tabRef.current = node;
        setNodeRef(node);
        setDropRef(node);
    }, [setNodeRef, setDropRef]);

    React.useEffect(() => {
        if (!isOver || !tabRef.current) return;
        const handleMouseMove = (e: MouseEvent) => {
            const rect = tabRef.current!.getBoundingClientRect();
            const mouseX = e.clientX;
            const tabCenter = rect.left + rect.width / 2;
            onDragOver(mouseX < tabCenter ? index : index + 1);
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
        <>
        <div
            ref={setRefs}
            style={style}
            {...attributes}
            {...listeners}
            data-tab-id={tab.id}
            onClick={onClick}
            onDoubleClick={(e) => {
              e.stopPropagation();
              if (isPreview) pinTab(layoutNodeId, tab.id);
            }}
            onContextMenu={(e) => {
              e.preventDefault();
              e.stopPropagation();
              setMenuPosition({ x: e.clientX, y: e.clientY });
            }}
            className={editorTabItemVariants({ active: isActive, dragging: isDragging, preview: isPreview })}
        >
            {isPreview ? (
              <Tooltip>
                <TooltipTrigger asChild>
                  <span className="max-w-[120px] truncate">{title}</span>
                </TooltipTrigger>
                <TooltipContent side="bottom">{t('tabBar.previewHint')}</TooltipContent>
              </Tooltip>
            ) : (
              <span className="max-w-[120px] truncate">{title}</span>
            )}
            {statusKey && statusIcon ? (
                <Tooltip>
                  <TooltipTrigger asChild>
                    <span className="ml-1 flex items-center">{statusIcon}</span>
                  </TooltipTrigger>
                  <TooltipContent side="bottom">{t(`tabBar.status.${statusKey}`)}</TooltipContent>
                </Tooltip>
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
        {menuPosition ? (
          <ContextMenu
            position={menuPosition}
            sections={buildTabContextMenuSections(layoutNodeId, tab, t, {
              revealInSidebar: () => onRevealInSidebar(),
            })}
            onClose={() => setMenuPosition(null)}
          />
        ) : null}
        </>
    );
});
