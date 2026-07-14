import { getEditorGroupActiveTabId, useEditorTabStore } from '@/features/core/layout/editorTabStore';
import { findTabUnderPointer } from '@/features/core/layout/editorDropTarget';
import { switchEditorTab } from './switchEditorTab';

/** VS Code `MultiEditorTabsControl.DRAG_OVER_OPEN_TAB_THRESHOLD` */
export const DRAG_OVER_OPEN_TAB_MS = 1500;

let hoverTimer: ReturnType<typeof setTimeout> | null = null;
let hoverKey: string | null = null;

export function clearTabDragHoverOpen(): void {
  if (hoverTimer) {
    clearTimeout(hoverTimer);
    hoverTimer = null;
  }
  hoverKey = null;
}

export function scheduleTabDragHoverOpen(pointerX: number, pointerY: number): void {
  const hovered = findTabUnderPointer(pointerX, pointerY);
  if (!hovered) {
    clearTabDragHoverOpen();
    return;
  }

  const activeTabId = getEditorGroupActiveTabId(hovered.groupId);
  if (activeTabId === hovered.tabId) {
    clearTabDragHoverOpen();
    return;
  }

  const key = `${hovered.groupId}:${hovered.tabId}`;
  if (hoverKey === key && hoverTimer) return;

  clearTabDragHoverOpen();
  hoverKey = key;
  hoverTimer = setTimeout(() => {
    hoverTimer = null;
    const tab = useEditorTabStore.getState().resolveTab(hovered.tabId);
    if (!tab) return;
    if (getEditorGroupActiveTabId(hovered.groupId) === hovered.tabId) return;
    // VS Code openEditor(..., { preserveFocus: true }) — preview tab without stealing drag focus
    useEditorTabStore.getState().setActiveTab(hovered.groupId, hovered.tabId);
    void switchEditorTab(hovered.groupId, tab);
  }, DRAG_OVER_OPEN_TAB_MS);
}
