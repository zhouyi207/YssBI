import React, { useRef, useEffect, useCallback, useState, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { VscWarning, VscSync, VscError } from "react-icons/vsc";
import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { orderTabsForTabBar } from "@/features/core/layout/tabBarOrder";
import { resolveTabDragTransferIds } from "@/features/core/layout/tabSelection";
import { useEditorTabStore } from "@/features/core/layout/editorTabStore";
import { computeTabShiftOffset } from "@/features/core/layout/tabBarInsertIndex";
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
  editorTabBarShellClass,
  editorTabBarStripClass,
  editorTabCloseButtonClass,
  editorTabItemVariants,
  editorTabReorderGapClass,
} from "./editorTabStyles";
import { EditorGroupToolbar } from "./EditorGroupToolbar";
import { DROP_TYPES, DRAG_TYPES } from "@/features/core/dnd";
import {
  closeTab,
  pinTab,
  switchTab,
  toggleMaximizeEditorGroup,
} from "@/features/application/editor/tabCommands";
import { useTabBarReorderStore, type TabBarReorderPreview } from "@/features/application/editor/tabBarReorderStore";
import { buildTabContextMenuSections } from "@/features/application/editor/tabContextMenu";
import { resolveTabDisplayName } from "@/features/application/editor/resolveTabDisplayName";
import { isPreviewLayoutTab } from "@/features/core/layout/layoutTabModel";
import { activateEditorGroup } from "@/features/application/editor/switchEditorTab";
import {
  bindEditorGroupStripPointerDown,
  bindTabDragPointerDown,
} from "@/features/application/editor/tabBarDragHandlers";
import { readEditorPartOptions } from "@/features/core/layout/editorPartOptions";
import { useSidebarTab } from "@/features/application/editor/useSidebarTab";
import { resourceKey, useDocumentStateStore, useResourceStore } from "@/features/core/resource";

interface TabBarProps {
  layoutNodeId: string;
  tabs: LayoutTab[];
  activeTabId?: string;
}

export const TabBar: React.FC<TabBarProps> = ({ layoutNodeId, tabs = [], activeTabId }) => {
  const switchSidebarTab = useSidebarTab();
  const { isDragging } = useLayoutStore(useShallow((s) => ({
    isDragging: s.isDragging,
  })));
  const displayTabs = useMemo(() => orderTabsForTabBar(tabs), [tabs]);
  const reorderPreview = useTabBarReorderStore((state) => state.preview);
  const showReorderPreview = reorderPreview?.targetGroupId === layoutNodeId;

  const containerRef = useRef<HTMLDivElement>(null);
  const { setNodeRef: setTabBarDropRef } = useDroppable({
    id: `tabbar-${layoutNodeId}`,
    data: { dropType: DROP_TYPES.TABBAR, targetNodeId: layoutNodeId, targetTabIndex: tabs.length },
  });

  const {
    listeners: groupDragListeners,
    setNodeRef: setGroupDragRef,
  } = useDraggable({
    id: `editor-group-drag-${layoutNodeId}`,
    data: { type: DRAG_TYPES.EDITOR_GROUP, sourceNodeId: layoutNodeId },
  });

  const handleGroupStripPointerDown = useCallback(
    bindEditorGroupStripPointerDown(groupDragListeners, () => {
      void activateEditorGroup(layoutNodeId);
    }),
    [groupDragListeners, layoutNodeId],
  );

  const handleTabStripClick = useCallback((e: React.MouseEvent) => {
    const target = (e.target as HTMLElement).closest<HTMLElement>('[data-tab-id]');
    if (!target) return;
    if ((e.target as HTMLElement).closest('button')) return;
    const tabId = target.dataset.tabId;
    if (!tabId) return;
    const tab = tabs.find((item) => item.id === tabId);
    if (!tab) return;
    if (e.ctrlKey || e.metaKey) {
      useEditorTabStore.getState().toggleTabInSelection(layoutNodeId, tabId);
      void activateEditorGroup(layoutNodeId);
      return;
    }
    useEditorTabStore.getState().setSelectedTabIds(layoutNodeId, [tabId]);
    void switchTab(layoutNodeId, tab.id, tab);
  }, [layoutNodeId, tabs]);

  const handleTabStripAuxClick = useCallback((e: React.MouseEvent) => {
    if (e.button !== 1) return;
    const target = (e.target as HTMLElement).closest<HTMLElement>('[data-tab-id]');
    if (!target) return;
    const tabId = target.dataset.tabId;
    if (!tabId) return;
    e.preventDefault();
    e.stopPropagation();
    void closeTab(layoutNodeId, tabId);
  }, [layoutNodeId]);

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
    const activeEl = container.querySelector(`[data-tab-id="${activeTabId}"]`) as HTMLElement | null;
    if (!activeEl) return;

    const tabRect = activeEl.getBoundingClientRect();
    const viewportRect = container.getBoundingClientRect();
    const isVisible =
      tabRect.left >= viewportRect.left
      && tabRect.right <= viewportRect.right;
    if (!isVisible) {
      activeEl.scrollIntoView({ behavior: 'auto', block: 'nearest', inline: 'nearest' });
    }
  }, [activeTabId]);

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

  const handleTabClose = useCallback((tabId: string, e: React.MouseEvent) => {
    e.stopPropagation();
    void closeTab(layoutNodeId, tabId);
  }, [layoutNodeId]);

  return (
    <div className={editorTabBarShellClass}>
      <div
        ref={setTabBarDropRef}
        data-tabbar-drop={layoutNodeId}
        className="relative flex min-h-0 flex-1 items-stretch h-full min-w-0"
      >
        <OverlayScrollbar
          ref={containerRef}
          direction="horizontal"
          className="flex min-h-0 flex-1 items-stretch h-full"
        >
          <div
            data-tab-strip={layoutNodeId}
            className={editorTabBarStripClass}
            onClick={handleTabStripClick}
            onAuxClick={handleTabStripAuxClick}
          >
            {displayTabs.map((tab, index) => (
              <TabItem
                key={tab.id}
                tab={tab}
                index={index}
                layoutNodeId={layoutNodeId}
                isActive={activeTabId === tab.id}
                reorderPreview={showReorderPreview ? reorderPreview : null}
                onClose={handleTabClose}
                onRevealInSidebar={() => revealInSidebar(tab)}
              />
            ))}
            {showReorderPreview && reorderPreview ? (
              <div
                className={editorTabReorderGapClass}
                style={{
                  left: reorderPreview.gapLeft,
                  width: reorderPreview.gapWidth,
                }}
              />
            ) : null}
            <div
              ref={setGroupDragRef}
              data-group-drag-fill={layoutNodeId}
              className="min-w-8 flex-1 self-stretch"
              onPointerDown={handleGroupStripPointerDown}
              aria-hidden="true"
            />
          </div>
        </OverlayScrollbar>
      </div>

      <EditorGroupToolbar groupId={layoutNodeId} />
    </div>
  );
};

interface TabItemProps {
  tab: LayoutTab;
  index: number;
  layoutNodeId: string;
  isActive: boolean;
  reorderPreview: TabBarReorderPreview | null;
  onClose: (tabId: string, e: React.MouseEvent) => void;
  onRevealInSidebar: () => void;
}

function areTabItemPropsEqual(prev: TabItemProps, next: TabItemProps): boolean {
  if (prev.tab.id !== next.tab.id) return false;
  if (prev.tab.pinned !== next.tab.pinned) return false;
  if (prev.isActive !== next.isActive) return false;
  if (prev.index !== next.index) return false;
  if (prev.layoutNodeId !== next.layoutNodeId) return false;
  if (prev.onClose !== next.onClose) return false;
  if (prev.onRevealInSidebar !== next.onRevealInSidebar) return false;
  if (prev.reorderPreview !== next.reorderPreview) return false;
  return true;
}

const TabItem: React.FC<TabItemProps> = React.memo(({
  tab, index, layoutNodeId, isActive, reorderPreview, onClose, onRevealInSidebar,
}) => {
  const { t } = useTranslation();
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
  const title = resourceTitle ?? baseTitle;

  const isPreview = isPreviewLayoutTab(tab);
  const isTabSelected = useEditorTabStore(
    (s) => s.getPlacement(layoutNodeId).selectedTabIds.includes(tab.id),
  );
  const draggedTabIds = resolveTabDragTransferIds(layoutNodeId, tab.id);

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

  const { attributes, listeners, setNodeRef, isDragging } = useDraggable({
    id: `tab-${layoutNodeId}-${tab.id}`,
    data: {
      type: DRAG_TYPES.TAB,
      tabId: tab.id,
      sourceNodeId: layoutNodeId,
      draggedTabIds: draggedTabIds.length > 1 ? draggedTabIds : undefined,
    },
  });

  const handleTabPointerDown = useCallback(
    bindTabDragPointerDown(listeners, () => {
      void activateEditorGroup(layoutNodeId);
    }),
    [listeners, layoutNodeId],
  );

  const shiftX = !isDragging && reorderPreview && reorderPreview.targetGroupId === layoutNodeId
    ? computeTabShiftOffset(
        index,
        reorderPreview.draggedIndex,
        reorderPreview.insertIndex,
        reorderPreview.gapWidth,
      )
    : 0;

  const style: React.CSSProperties = {
    transform: shiftX !== 0 ? `translate3d(${shiftX}px, 0, 0)` : undefined,
    height: 'var(--titlebar-height)',
  };

  return (
    <>
      <div
        ref={setNodeRef}
        style={style}
        {...attributes}
        onPointerDown={handleTabPointerDown}
        data-tab-id={tab.id}
        data-tab-group={layoutNodeId}
        data-tab-title={title}
        onDoubleClick={(e) => {
          e.stopPropagation();
          if (isPreview || tab.pinned === false) {
            void pinTab(layoutNodeId, tab.id);
            return;
          }
          const dblClickMode = readEditorPartOptions().doubleClickTabToToggleEditorGroupSizes;
          if (dblClickMode === 'off') return;
          void toggleMaximizeEditorGroup(layoutNodeId);
        }}
        onContextMenu={(e) => {
          e.preventDefault();
          e.stopPropagation();
          setMenuPosition({ x: e.clientX, y: e.clientY });
        }}
        className={editorTabItemVariants({
          active: isActive,
          dragging: isDragging,
          preview: isPreview,
          selected: isTabSelected && !isActive,
        })}
      >
        {isPreview ? (
          <Tooltip>
            <TooltipTrigger asChild>
              <span className="max-w-[120px] truncate leading-normal">{title}</span>
            </TooltipTrigger>
            <TooltipContent side="bottom">{t('tabBar.previewHint')}</TooltipContent>
          </Tooltip>
        ) : (
          <span className="max-w-[120px] truncate leading-normal">{title}</span>
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
          data-dirty={isDirty ? 'true' : 'false'}
          onClick={(e) => onClose(tab.id, e)}
          className={editorTabCloseButtonClass}
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
}, areTabItemPropsEqual);
