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

/**
 * Unified editor tab activation: graph reload, worksheet detail, layout selection, session.
 */
export async function switchEditorTab(groupId: string, tab: LayoutTab): Promise<boolean> {
  useLayoutStore.getState().setActiveGroup(groupId);
  applyEditorTabSelection(groupId, tab.id);

  if (tab.type === 'event' || tab.type === 'function') {
    useEditorStore.getState().setDetailFocus({ kind: tab.type, path: tab.id });
    ensureDetailVisible();
    const loaded = await activateGraphTab(tab.id, groupId);
    if (!loaded) return false;
    ensureGraphViewport(tab.id, getGraphByPath(tab.id)?.canvas);
    syncVariablesGraphScopeFromActiveTab();
    return true;
  }

  if (tab.type === 'worksheet') {
    useEditorStore.getState().setDetailFocus({ kind: 'worksheet', id: tab.id });
    ensureDetailVisible();
    return true;
  }

  return true;
}

/** Restore session + backend load after close without changing detail focus. */
export async function activateCurrentEditorTab(groupId: string): Promise<boolean> {
  const node = useLayoutStore.getState().nodes[groupId];
  const activeTabId = node?.data?.activeTabId;
  if (!activeTabId) {
    useGraphSessionStore.getState().clearGroupActivePath(groupId);
    return false;
  }
  const activeTab = node?.data?.tabs?.find((tab) => tab.id === activeTabId);
  if (!activeTab) {
    useGraphSessionStore.getState().clearGroupActivePath(groupId);
    return false;
  }

  if (activeTab.type === 'event' || activeTab.type === 'function') {
    const loaded = await activateGraphTab(activeTab.id, groupId);
    if (!loaded) return false;
    ensureGraphViewport(activeTab.id, getGraphByPath(activeTab.id)?.canvas);
    syncVariablesGraphScopeFromActiveTab();
    return true;
  }

  if (activeTab.type === 'worksheet') {
    return true;
  }

  useGraphSessionStore.getState().clearGroupActivePath(groupId);
  return false;
}
