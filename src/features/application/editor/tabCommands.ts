import { editorDockviewPort } from '@/features/core/dockview';
import { locateLayoutTab } from '@/features/core/layout/layoutTabQueries';
import { isGraphResourceDirty } from '@/features/core/resource';
import type { LayoutTab } from '@/shared/types/ui';
import { splitEditorAtEdge } from './editorGroupCommands';
import { closeEditorTab } from './closeEditorTab';
import { activateEditorGroup, switchEditorTab } from './switchEditorTab';
import { layoutTabFromDockviewPanel, listDockviewGroupPanels } from './dockviewTabProjection';

async function activateDockviewGroup(groupId: string): Promise<void> {
  if (editorDockviewPort.getActiveGroupId() === groupId) return;
  await activateEditorGroup(groupId);
}

export async function switchTab(
  groupId: string,
  tabId: string,
  tab?: LayoutTab | null,
): Promise<boolean> {
  const resolved = tab?.id === tabId ? tab : locateLayoutTab(tabId, groupId)?.tab;
  if (!resolved) return false;
  return switchEditorTab(groupId, resolved);
}

export async function closeTab(
  groupId: string,
  tabId: string,
  skipDirtyPrompt = false,
): Promise<boolean> {
  await activateDockviewGroup(groupId);
  return closeEditorTab(tabId, groupId, skipDirtyPrompt);
}

export async function closeOtherTabs(groupId: string, keepTabId: string): Promise<boolean> {
  await activateDockviewGroup(groupId);
  for (const panel of listDockviewGroupPanels(groupId)) {
    const tab = layoutTabFromDockviewPanel(panel);
    if (!tab || tab.id === keepTabId) continue;
    if (!await closeEditorTab(tab.id, groupId)) return false;
  }
  return true;
}

export async function closeAllTabsInGroup(groupId: string): Promise<boolean> {
  await activateDockviewGroup(groupId);
  for (const panel of listDockviewGroupPanels(groupId)) {
    const tab = layoutTabFromDockviewPanel(panel);
    if (tab && !await closeEditorTab(tab.id, groupId)) return false;
  }
  return true;
}

export async function closeSavedTabsInGroup(groupId: string): Promise<boolean> {
  await activateDockviewGroup(groupId);
  for (const panel of listDockviewGroupPanels(groupId)) {
    const tab = layoutTabFromDockviewPanel(panel);
    if (!tab || (tab.type !== 'event' && tab.type !== 'function' && tab.type !== 'worksheet')) continue;
    if (isGraphResourceDirty(tab.id, tab.type)) continue;
    if (!await closeEditorTab(tab.id, groupId, true)) return false;
  }
  return true;
}

export async function closeEditorGroup(groupId: string): Promise<boolean> {
  return closeAllTabsInGroup(groupId);
}

export async function splitEditorGroup(groupId: string, direction: 'row' | 'col' = 'row'): Promise<void> {
  await activateDockviewGroup(groupId);
  await splitEditorAtEdge(groupId, direction === 'row' ? 'right' : 'bottom');
}

export async function splitEditorGroupFromPointer(groupId: string, altPressed: boolean): Promise<void> {
  await splitEditorAtEdge(groupId, altPressed ? 'bottom' : 'right');
}

/** Dockview tabs are persistent panels; preview pinning is no longer application-owned. */
export async function pinTab(groupId: string, tabId: string): Promise<void> {
  await activateDockviewGroup(groupId);
  const panel = editorDockviewPort
    .findPanelsByResource(tabId)
    .find((candidate) => candidate.groupId === groupId);
  if (panel) await editorDockviewPort.activate(panel.panelInstanceId);
}

/** Sticky ordering belonged to the removed custom TabBar and is intentionally unsupported. */
export async function setTabSticky(groupId: string, _tabId: string, _sticky: boolean): Promise<void> {
  await activateDockviewGroup(groupId);
}

/** Group locking belonged to the removed custom editor grid. */
export function toggleEditorGroupLocked(_groupId: string): void {}

/** Dockview owns group sizing; the legacy maximize command is retained as a compatibility no-op. */
export async function toggleMaximizeEditorGroup(groupId: string): Promise<void> {
  await activateDockviewGroup(groupId);
}

export function locateTab(tabId: string, groupId?: string) {
  return locateLayoutTab(tabId, groupId);
}
