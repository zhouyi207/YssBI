import {
  computeTabGapLeft,
  computeTabInsertIndex,
  measureTabBarMetrics,
  resolveTabGapWidth,
} from '@/features/core/layout/tabBarInsertIndex';
import { listEditorGroupTabIds } from '@/features/core/layout/editorTabStore';
import type { TabBarReorderPreview } from './tabBarReorderStore';

const TAB_BAR_DROP_SELECTOR = '[data-tabbar-drop]';
const TAB_STRIP_SELECTOR = '[data-tab-strip]';

export interface TabBarInsertPreviewContext {
  /** Existing tab being reordered; null for external insert (e.g. sidebar graph). */
  draggedTabId: string | null;
  /** Source editor group for tab reorder; null for external insert. */
  sourceGroupId: string | null;
}

export function findTabBarTargetFromPointer(
  pointerX: number,
  pointerY: number,
): { groupId: string; stripElement: HTMLElement } | null {
  const dropTargets = document.querySelectorAll<HTMLElement>(TAB_BAR_DROP_SELECTOR);
  for (const dropElement of dropTargets) {
    const rect = dropElement.getBoundingClientRect();
    if (
      pointerX >= rect.left
      && pointerX <= rect.right
      && pointerY >= rect.top
      && pointerY <= rect.bottom
    ) {
      const groupId = dropElement.dataset.tabbarDrop;
      if (!groupId) continue;
      const stripElement = dropElement.querySelector<HTMLElement>(TAB_STRIP_SELECTOR) ?? dropElement;
      return { groupId, stripElement };
    }
  }
  return null;
}

function tabIdsForGroup(groupId: string): string[] {
  return listEditorGroupTabIds(groupId);
}

/** Build TabBar gap preview for tab reorder or external graph insert. */
export function buildTabBarInsertPreview(
  groupId: string,
  stripElement: HTMLElement,
  pointerX: number,
  context: TabBarInsertPreviewContext,
): TabBarReorderPreview {
  const { draggedTabId, sourceGroupId } = context;
  const tabIds = tabIdsForGroup(groupId);
  const metrics = measureTabBarMetrics(stripElement, tabIds);

  if (metrics.length === 0) {
    return {
      targetGroupId: groupId,
      sourceGroupId,
      draggedTabId,
      insertIndex: 0,
      draggedIndex: -1,
      gapWidth: resolveTabGapWidth(metrics, draggedTabId),
      gapLeft: 0,
    };
  }

  const sameGroup = Boolean(sourceGroupId && sourceGroupId === groupId);
  const draggedIndex = sameGroup && draggedTabId
    ? metrics.findIndex((metric) => metric.tabId === draggedTabId)
    : -1;
  const scrollLeft = (
    stripElement.closest('.overlay-scrollbar-viewport') as HTMLElement | null
  )?.scrollLeft ?? 0;
  const insertIndex = computeTabInsertIndex(
    pointerX - stripElement.getBoundingClientRect().left + scrollLeft,
    metrics,
  );
  const gapWidth = resolveTabGapWidth(metrics, sameGroup ? draggedTabId : null);

  return {
    targetGroupId: groupId,
    sourceGroupId,
    draggedTabId,
    insertIndex,
    draggedIndex,
    gapWidth,
    gapLeft: computeTabGapLeft(metrics, insertIndex, sameGroup ? draggedTabId : null),
  };
}
