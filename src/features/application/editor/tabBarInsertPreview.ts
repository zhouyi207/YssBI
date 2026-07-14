import {
  computeTabGapLeft,
  computeTabInsertIndex,
  measureTabBarMetrics,
  resolveTabGapWidth,
} from '@/features/core/layout/tabBarInsertIndex';
import {
  findTabBarTargetFromPointer,
  type TabBarInsertPreviewContext,
} from '@/features/core/layout/editorDropTarget';
import { listEditorGroupTabIds } from '@/features/core/layout/editorTabStore';
import type { TabBarReorderPreview } from './tabBarReorderStore';

export { findTabBarTargetFromPointer };
export type { TabBarInsertPreviewContext };

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
