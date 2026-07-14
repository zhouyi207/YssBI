import { useEditorStore } from '@/features/core/editor';
import { syncVariablesGraphScopeFromActiveTab } from '@/features/core/editor/detail/variablesGraphScope';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { getEditorGroupActiveTabId, useEditorTabStore } from '@/features/core/layout/editorTabStore';
import type { LayoutTab } from '@/shared/types/ui';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { activateGraphTab } from './activateGraphTab';
import { applyEditorTabSelection } from './editorTabSelection';
import { ensureDetailVisible } from './ensureDetailVisible';
import { suspendEditorGroupGraphSession } from './graphSessionLifecycle';

let editorGroupSessionChain: Promise<void> = Promise.resolve();

function scheduleSuspendPreviousGroup(prevGroupId: string): void {
  editorGroupSessionChain = editorGroupSessionChain
    .then(() => suspendEditorGroupGraphSession(prevGroupId))
    .catch(() => undefined);
}

/**
 * VS Code MOUSE_DOWN — update active editor group immediately.
 * Previous group session suspend runs on a serialized background chain.
 */
export function focusEditorGroupSync(groupId: string): boolean {
  const prevGroupId = useLayoutStore.getState().activeEditorGroupId;
  if (prevGroupId === groupId) return false;
  useLayoutStore.getState().setActiveGroup(groupId);
  if (prevGroupId) {
    scheduleSuspendPreviousGroup(prevGroupId);
  }
  return true;
}

export async function awaitEditorGroupSessionChain(): Promise<void> {
  await editorGroupSessionChain;
}

/** Load the group's current tab session after any pending suspend work. */
export async function hydrateEditorGroup(groupId: string): Promise<boolean> {
  await editorGroupSessionChain;
  return activateCurrentEditorTab(groupId);
}

async function focusEditorGroup(groupId: string): Promise<void> {
  focusEditorGroupSync(groupId);
  await editorGroupSessionChain;
}

/**
 * Unified editor tab activation: graph reload, worksheet detail, layout selection, session.
 */
export async function switchEditorTab(groupId: string, tab: LayoutTab): Promise<boolean> {
  await focusEditorGroup(groupId);
  applyEditorTabSelection(groupId, tab.id);

  if (tab.type === 'event' || tab.type === 'function') {
    useEditorStore.getState().setDetailFocus({ kind: tab.type, path: tab.id });
    ensureDetailVisible();
    const loaded = await activateGraphTab(tab.id, groupId);
    if (!loaded) return false;
    syncVariablesGraphScopeFromActiveTab();
    return true;
  }

  if (tab.type === 'worksheet') {
    useEditorStore.getState().setDetailFocus({ kind: 'worksheet', id: tab.id });
    ensureDetailVisible();
    const sessionStore = useGraphSessionStore.getState();
    if (sessionStore.getFocusedGroupId() === groupId) {
      sessionStore.clearFocusedSession(groupId);
    }
    return true;
  }

  return true;
}

/** Restore session + backend load after close without changing detail focus. */
export async function activateCurrentEditorTab(groupId: string): Promise<boolean> {
  const activeTabId = getEditorGroupActiveTabId(groupId);
  if (!activeTabId) {
    useGraphSessionStore.getState().clearFocusedSession(groupId);
    return false;
  }
  const activeTab = useEditorTabStore.getState().resolveTab(activeTabId);
  if (!activeTab) {
    useGraphSessionStore.getState().clearFocusedSession(groupId);
    return false;
  }

  if (activeTab.type === 'event' || activeTab.type === 'function') {
    const loaded = await activateGraphTab(activeTab.id, groupId);
    if (!loaded) return false;
    syncVariablesGraphScopeFromActiveTab();
    return true;
  }

  if (activeTab.type === 'worksheet') {
    return true;
  }

  useGraphSessionStore.getState().clearFocusedSession(groupId);
  return false;
}

/** Activate an editor group and hydrate its current graph-backed session as one application action. */
export async function activateEditorGroup(groupId: string): Promise<boolean> {
  focusEditorGroupSync(groupId);
  return hydrateEditorGroup(groupId);
}
