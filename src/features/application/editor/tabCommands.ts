import { locateLayoutTab } from '@/features/core/layout/layoutTabQueries';
import { splitEditorAtEdge } from '@/features/application/editor/editorGroupCommands';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { listEditorGroupTabIds, useEditorTabStore } from '@/features/core/layout/editorTabStore';
import { EditorGroupsService } from '@/features/core/layout/editorGroupsService';
import { isGraphResourceDirty } from '@/features/core/resource';
import type { LayoutTab } from '@/shared/types/ui';
import { closeEditorTab } from './closeEditorTab';
import { activateEditorGroup, switchEditorTab } from './switchEditorTab';

async function activateTabBarGroup(groupId: string): Promise<void> {
  if (useLayoutStore.getState().activeEditorGroupId === groupId) return;
  await activateEditorGroup(groupId);
}

export async function switchTab(
  groupId: string,
  tabId: string,
  tab?: LayoutTab | null,
): Promise<boolean> {
  const resolved =
    tab && tab.id === tabId
      ? tab
      : useEditorTabStore.getState().resolveTab(tabId);
  if (!resolved) return false;
  return switchEditorTab(groupId, resolved);
}

export async function closeTab(
  groupId: string,
  tabId: string,
  skipDirtyPrompt = false,
): Promise<boolean> {
  await activateTabBarGroup(groupId);
  return closeEditorTab(tabId, groupId, skipDirtyPrompt);
}

export async function closeOtherTabs(groupId: string, keepTabId: string): Promise<boolean> {
  await activateTabBarGroup(groupId);
  const tabIds = listEditorGroupTabIds(groupId);
  for (const tabId of tabIds) {
    if (tabId === keepTabId) continue;
    const closed = await closeEditorTab(tabId, groupId);
    if (!closed) return false;
  }
  return true;
}

export async function closeAllTabsInGroup(groupId: string): Promise<boolean> {
  await activateTabBarGroup(groupId);
  const tabIds = listEditorGroupTabIds(groupId);
  for (const tabId of [...tabIds]) {
    const closed = await closeEditorTab(tabId, groupId);
    if (!closed) return false;
  }
  return true;
}

export async function closeSavedTabsInGroup(groupId: string): Promise<boolean> {
  await activateTabBarGroup(groupId);
  const tabs = useEditorTabStore.getState().resolveGroupTabs(groupId);
  for (const tab of [...tabs]) {
    if (tab.type !== 'event' && tab.type !== 'function' && tab.type !== 'worksheet') continue;
    if (isGraphResourceDirty(tab.id, tab.type)) continue;
    const closed = await closeEditorTab(tab.id, groupId, true);
    if (!closed) return false;
  }
  return true;
}

export async function closeEditorGroup(groupId: string): Promise<boolean> {
  await activateTabBarGroup(groupId);
  const tabIds = listEditorGroupTabIds(groupId);
  if (tabIds.length === 0) {
    useLayoutStore.getState().removeEditorGroup(groupId);
    return true;
  }
  for (const tabId of [...tabIds]) {
    const closed = await closeEditorTab(tabId, groupId);
    if (!closed) return false;
  }
  return true;
}

export async function splitEditorGroup(groupId: string, direction: 'row' | 'col' = 'row'): Promise<void> {
  await activateTabBarGroup(groupId);
  await splitEditorAtEdge(groupId, direction === 'row' ? 'right' : 'bottom');
}

export async function splitEditorGroupFromPointer(groupId: string, altPressed: boolean): Promise<void> {
  await splitEditorGroup(groupId, altPressed ? 'col' : 'row');
}

/** Pin a preview tab so it is no longer replaced by sidebar preview opens. */
export async function pinTab(groupId: string, tabId: string): Promise<void> {
  await activateTabBarGroup(groupId);
  useLayoutStore.getState().setTabPinned(groupId, tabId, true);
}

export async function toggleMaximizeEditorGroup(groupId: string): Promise<void> {
  await activateTabBarGroup(groupId);
  EditorGroupsService.toggleMaximizeGroup(groupId);
}

export function locateTab(tabId: string, groupId?: string) {
  return locateLayoutTab(tabId, groupId);
}
