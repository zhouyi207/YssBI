import type { EditorSplitEdge } from '@/features/core/layout/editorSplitLayout';
import { EditorGroupsService } from '@/features/core/layout/editorGroupsService';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useEditorTabStore } from '@/features/core/layout/editorTabStore';
import { getActiveLayoutTab } from '@/features/core/layout/layoutTabQueries';
import { readEditorPartOptions } from '@/features/core/layout/editorPartOptions';
import { canMergeEditorGroup, canMoveTabsAcrossEditorGroups } from '@/features/core/layout/editorGroupLock';
import { splitComponentForTab } from '@/features/core/layout/layoutTabModel';
import { uiStore } from '@/features/core/ui/UIStore';
import { i18n } from '@/app/i18n';
import { switchEditorTab } from './switchEditorTab';

async function activateCreatedEditorGroup(groupId: string | null): Promise<string | null> {
  if (!groupId) return null;
  const activeTab = getActiveLayoutTab(groupId)?.tab;
  if (activeTab) await switchEditorTab(groupId, activeTab);
  return groupId;
}

/** Move one or more tabs onto another editor group's TabBar (removes from source). */
export function moveTabsBetweenGroups(
  sourceGroupId: string,
  tabIds: string[],
  targetGroupId: string,
  targetTabIndex?: number,
): void {
  if (!canMoveTabsAcrossEditorGroups(sourceGroupId, targetGroupId)) {
    uiStore.showToast(i18n.t('tabBar.lockedGroupMoveBlocked'), 'warning');
    return;
  }
  const wasInactive = useLayoutStore.getState().activeEditorGroupId !== targetGroupId;
  useEditorTabStore.getState().moveTabs(sourceGroupId, tabIds, targetGroupId, targetTabIndex);
  useLayoutStore.getState().setActiveGroup(targetGroupId);
  if (!wasInactive && sourceGroupId === targetGroupId) return;
  const activeTab = getActiveLayoutTab(targetGroupId)?.tab;
  if (activeTab) void switchEditorTab(targetGroupId, activeTab);
}

/** Move a tab onto another editor group's TabBar (removes from source). */
export function moveTabBetweenGroups(
  sourceGroupId: string,
  tabId: string,
  targetGroupId: string,
  targetTabIndex?: number,
): void {
  moveTabsBetweenGroups(sourceGroupId, [tabId], targetGroupId, targetTabIndex);
}

/** VS Code copy editor — duplicate tab reference into target group. */
export function copyTabsBetweenGroups(
  _sourceGroupId: string,
  tabIds: string[],
  targetGroupId: string,
  targetTabIndex?: number,
): void {
  let insertAt = targetTabIndex;
  for (const tabId of tabIds) {
    useEditorTabStore.getState().duplicateTabReference(targetGroupId, tabId, insertAt);
    if (insertAt !== undefined) insertAt += 1;
  }
  useLayoutStore.getState().setActiveGroup(targetGroupId);
  const activeTab = getActiveLayoutTab(targetGroupId)?.tab;
  if (activeTab) void switchEditorTab(targetGroupId, activeTab);
}

/** VS Code copy editor — duplicate tab reference into target group. */
export function copyTabBetweenGroups(
  sourceGroupId: string,
  tabId: string,
  targetGroupId: string,
  targetTabIndex?: number,
): void {
  copyTabsBetweenGroups(sourceGroupId, [tabId], targetGroupId, targetTabIndex);
}

/**
 * Drag tab to editor edge split:
 * - Multiple tabs in source → move dragged tab to the new group.
 * - Single tab in source → copy (keep source group alive; moving empties it and breaks the grid).
 */
export async function splitEditorWithTab(
  sourceGroupId: string,
  tabId: string,
  targetGroupId: string,
  edge: EditorSplitEdge,
  options?: { copy?: boolean },
): Promise<string | null> {
  if (!canMoveTabsAcrossEditorGroups(sourceGroupId, targetGroupId)) {
    uiStore.showToast(i18n.t('tabBar.lockedGroupMoveBlocked'), 'warning');
    return null;
  }
  const sourceTabs = useEditorTabStore.getState().resolveGroupTabs(sourceGroupId);
  const tab = sourceTabs.find((t) => t.id === tabId);
  if (!tab) return null;

  const copy = options?.copy ?? false;
  const moveFromSource = !copy && sourceTabs.length > 1;

  const created = EditorGroupsService.splitGroupAtEdge(targetGroupId, edge, {
    component: tab.component || 'GraphEditor',
    tabs: [{ ...tab, pinned: true as const }],
    activeTabId: tabId,
  });
  if (!created) return null;

  if (moveFromSource) {
    const sourceStillHasTab = useEditorTabStore
      .getState()
      .getPlacement(sourceGroupId)
      .tabIds
      .includes(tabId);
    if (sourceStillHasTab) {
      useLayoutStore.getState().removeTab(sourceGroupId, tabId);
    }
  }

  return activateCreatedEditorGroup(created);
}

/** VS Code mergeGroup — move all tabs from source into target. */
export function mergeEditorGroupInto(
  sourceGroupId: string,
  targetGroupId: string,
  insertIndex?: number,
): void {
  if (!canMergeEditorGroup(sourceGroupId, targetGroupId)) {
    uiStore.showToast(i18n.t('tabBar.lockedGroupMoveBlocked'), 'warning');
    return;
  }
  useLayoutStore.getState().mergeEditorGroup(sourceGroupId, targetGroupId, insertIndex);
  const activeTab = getActiveLayoutTab(targetGroupId)?.tab;
  if (activeTab) void switchEditorTab(targetGroupId, activeTab);
}

/** VS Code copyGroup on merge drop — duplicate tabs without removing source group. */
export function copyEditorGroupInto(
  sourceGroupId: string,
  targetGroupId: string,
  insertIndex?: number,
): void {
  if (sourceGroupId === targetGroupId) return;
  useEditorTabStore.getState().duplicateGroupTabs(sourceGroupId, targetGroupId, insertIndex);
  useLayoutStore.getState().setActiveGroup(targetGroupId);
  const activeTab = getActiveLayoutTab(targetGroupId)?.tab;
  if (activeTab) void switchEditorTab(targetGroupId, activeTab);
}

/** VS Code moveGroup + addGroup split. */
export async function splitEditorGroupWithGroup(
  sourceGroupId: string,
  targetGroupId: string,
  edge: EditorSplitEdge,
): Promise<string | null> {
  if (!canMoveTabsAcrossEditorGroups(sourceGroupId, targetGroupId)) {
    uiStore.showToast(i18n.t('tabBar.lockedGroupMoveBlocked'), 'warning');
    return null;
  }
  const created = useLayoutStore.getState().splitEditorGroupWithGroup(
    sourceGroupId,
    targetGroupId,
    edge,
  );
  return activateCreatedEditorGroup(created);
}

/** VS Code copyGroup + addGroup split. */
export async function copyEditorGroupWithSplit(
  sourceGroupId: string,
  targetGroupId: string,
  edge: EditorSplitEdge,
): Promise<string | null> {
  const nodes = useLayoutStore.getState().nodes;
  const sourceTabs = useEditorTabStore.getState().resolveGroupTabs(sourceGroupId);
  const activeTab = getActiveLayoutTab(sourceGroupId, nodes)?.tab;

  const created = EditorGroupsService.splitGroupAtEdge(targetGroupId, edge, {
    component: splitComponentForTab(activeTab) || sourceTabs[0]?.component || 'GraphEditor',
    tabs: [],
    activeTabId: activeTab?.id,
  });
  if (!created) return null;

  useEditorTabStore.getState().duplicateGroupTabs(sourceGroupId, created);
  return activateCreatedEditorGroup(created);
}

/** Single-tab source + split + move (VS Code closeEmptyGroups optimization). */
export async function splitOrMoveSingleTabGroup(
  sourceGroupId: string,
  tabId: string,
  targetGroupId: string,
  edge: EditorSplitEdge,
): Promise<string | null> {
  const sourceTabs = useEditorTabStore.getState().resolveGroupTabs(sourceGroupId);
  if (sourceTabs.length !== 1) {
    return splitEditorWithTab(sourceGroupId, tabId, targetGroupId, edge);
  }

  const closeEmptyGroups = readEditorPartOptions().closeEmptyGroups;
  if (closeEmptyGroups) {
    return splitEditorGroupWithGroup(sourceGroupId, targetGroupId, edge);
  }
  return splitEditorWithTab(sourceGroupId, tabId, targetGroupId, edge);
}

/** Button / command split — copies active tab to right or bottom. */
export async function splitEditorAtEdge(
  groupId: string,
  edge: 'right' | 'bottom',
): Promise<string | null> {
  const created = edge === 'right'
    ? EditorGroupsService.splitActiveTabRight(groupId)
    : EditorGroupsService.splitActiveTabDown(groupId);
  return activateCreatedEditorGroup(created);
}
