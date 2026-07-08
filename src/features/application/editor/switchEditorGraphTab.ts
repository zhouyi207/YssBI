import { getGraphByPath } from '@/features/core/dataStore';
import { useEditorStore } from '@/features/core/editor';
import { syncVariablesGraphScopeFromActiveTab } from '@/features/core/editor/detail/variablesGraphScope';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { ensureGraphViewport } from '@/features/core/viewport';
import type { LayoutTab } from '@/shared/types/ui';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { activateGraphTab } from './activateGraphTab';
import { applyEditorTabSelection } from './editorTabSelection';
import { ensureDetailVisible } from './ensureDetailVisible';

type GraphTabType = Extract<LayoutTab['type'], 'event' | 'function'>;

function resolveTab(
  groupId: string,
  tabId: string,
  tab?: Pick<LayoutTab, 'type' | 'id'> | null,
): LayoutTab | undefined {
  if (tab && tab.id === tabId) {
    return tab as LayoutTab;
  }
  return useLayoutStore.getState().nodes[groupId]?.data?.tabs?.find((item) => item.id === tabId);
}

/**
 * Single entry for switching to a graph tab: layout selection, session activate,
 * backend reload, viewport, detail focus, and variable scope.
 */
export async function switchEditorGraphTab(
  groupId: string,
  tabId: string,
  tab?: Pick<LayoutTab, 'type' | 'id'> | null,
): Promise<boolean> {
  useLayoutStore.getState().setActiveGroup(groupId);
  applyEditorTabSelection(groupId, tabId);

  const resolvedTab = resolveTab(groupId, tabId, tab);
  if (resolvedTab?.type === 'event' || resolvedTab?.type === 'function') {
    useEditorStore.getState().setDetailFocus({ kind: resolvedTab.type, path: tabId });
    ensureDetailVisible();
  } else if (resolvedTab?.type === 'worksheet') {
    useEditorStore.getState().setDetailFocus({ kind: 'worksheet', id: tabId });
    ensureDetailVisible();
  }

  if (resolvedTab?.type === 'event' || resolvedTab?.type === 'function') {
    const loaded = await activateGraphTab(tabId, groupId);
    if (!loaded) return false;
    const tabSource = getGraphByPath(tabId);
    ensureGraphViewport(tabId, tabSource?.canvas);
    syncVariablesGraphScopeFromActiveTab();
  }

  return true;
}

export type { GraphTabType };

/** Restore session + backend load after close without changing detail focus. */
export async function activateCurrentGraphTab(groupId: string): Promise<boolean> {
  const node = useLayoutStore.getState().nodes[groupId];
  const activeTabId = node?.data?.activeTabId;
  if (!activeTabId) {
    useGraphSessionStore.getState().clearGroupActivePath(groupId);
    return false;
  }
  const activeTab = node?.data?.tabs?.find((tab) => tab.id === activeTabId);
  if (!activeTab || (activeTab.type !== 'event' && activeTab.type !== 'function')) {
    useGraphSessionStore.getState().clearGroupActivePath(groupId);
    return false;
  }
  const loaded = await activateGraphTab(activeTab.id, groupId);
  if (!loaded) return false;
  const tabSource = getGraphByPath(activeTab.id);
  ensureGraphViewport(activeTab.id, tabSource?.canvas);
  syncVariablesGraphScopeFromActiveTab();
  return true;
}
