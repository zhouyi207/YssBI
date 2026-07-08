import { getActiveLayoutTab, locateLayoutTab } from '@/features/core/layout/layoutTabQueries';
import { splitComponentForTab } from '@/features/core/layout/layoutTabModel';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { isGraphResourceDirty } from '@/features/core/resource';
import type { LayoutTab } from '@/shared/types/ui';
import { closeEditorTab } from './closeEditorTab';
import { switchEditorTab } from './switchEditorTab';

export async function switchTab(
  groupId: string,
  tabId: string,
  tab?: LayoutTab | null,
): Promise<boolean> {
  const resolved =
    tab && tab.id === tabId
      ? tab
      : useLayoutStore.getState().nodes[groupId]?.data?.tabs?.find((item) => item.id === tabId);
  if (!resolved) return false;
  return switchEditorTab(groupId, resolved);
}

export async function closeTab(
  groupId: string,
  tabId: string,
  skipDirtyPrompt = false,
): Promise<boolean> {
  return closeEditorTab(tabId, groupId, skipDirtyPrompt);
}

export async function closeOtherTabs(groupId: string, keepTabId: string): Promise<boolean> {
  const tabIds =
    useLayoutStore.getState().nodes[groupId]?.data?.tabs?.map((tab) => tab.id) ?? [];
  for (const tabId of tabIds) {
    if (tabId === keepTabId) continue;
    const closed = await closeEditorTab(tabId, groupId);
    if (!closed) return false;
  }
  return true;
}

export async function closeAllTabsInGroup(groupId: string): Promise<boolean> {
  const tabIds =
    useLayoutStore.getState().nodes[groupId]?.data?.tabs?.map((tab) => tab.id) ?? [];
  for (const tabId of [...tabIds]) {
    const closed = await closeEditorTab(tabId, groupId);
    if (!closed) return false;
  }
  return true;
}

export async function closeSavedTabsInGroup(groupId: string): Promise<boolean> {
  const tabs = useLayoutStore.getState().nodes[groupId]?.data?.tabs ?? [];
  for (const tab of [...tabs]) {
    if (tab.type !== 'event' && tab.type !== 'function' && tab.type !== 'worksheet') continue;
    if (isGraphResourceDirty(tab.id, tab.type)) continue;
    const closed = await closeEditorTab(tab.id, groupId, true);
    if (!closed) return false;
  }
  return true;
}

export async function closeEditorGroup(groupId: string): Promise<boolean> {
  const tabIds =
    useLayoutStore.getState().nodes[groupId]?.data?.tabs?.map((tab) => tab.id) ?? [];
  for (const tabId of [...tabIds]) {
    const closed = await closeEditorTab(tabId, groupId);
    if (!closed) return false;
  }
  useLayoutStore.getState().removeNode(groupId);
  return true;
}

export function splitEditorGroup(groupId: string, direction: 'row' | 'col' = 'row'): void {
  const nodes = useLayoutStore.getState().nodes;
  const activeTab = getActiveLayoutTab(groupId, nodes)?.tab;
  useLayoutStore.getState().splitNode(groupId, direction, splitComponentForTab(activeTab));
}

export function splitEditorGroupFromPointer(groupId: string, altPressed: boolean): void {
  splitEditorGroup(groupId, altPressed ? 'col' : 'row');
}

export function findTabInGroup(groupId: string, tabId: string): LayoutTab | undefined {
  return useLayoutStore.getState().nodes[groupId]?.data?.tabs?.find((tab) => tab.id === tabId);
}

/** Pin a preview tab so it is no longer replaced by sidebar preview opens. */
export function pinTab(groupId: string, tabId: string): void {
  useLayoutStore.getState().setTabPinned(groupId, tabId, true);
}

export function locateTab(tabId: string, groupId?: string) {
  return locateLayoutTab(tabId, groupId);
}
