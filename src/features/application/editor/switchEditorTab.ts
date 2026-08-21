import { syncVariablesGraphScopeFromActiveTab } from '@/features/core/editor/detail/variablesGraphScope';
import { editorDockviewPort } from '@/features/core/dockview';
import { getActiveLayoutTab } from '@/features/core/layout/layoutTabQueries';
import type { LayoutTab } from '@/shared/types/ui';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { activateGraphTab } from './activateGraphTab';
import { suspendEditorGroupGraphSession } from './graphSessionLifecycle';
import { focusDetails } from './rightSidebarActions';

let editorGroupSessionChain: Promise<void> = Promise.resolve();
let latestTabSwitchRequest = 0;
const pendingGroupSuspensions = new Set<string>();

function scheduleSuspendPreviousGroup(prevGroupId: string): void {
  if (pendingGroupSuspensions.has(prevGroupId)) return;
  pendingGroupSuspensions.add(prevGroupId);
  editorGroupSessionChain = editorGroupSessionChain
    .then(() => suspendEditorGroupGraphSession(prevGroupId))
    .catch(() => undefined)
    .finally(() => pendingGroupSuspensions.delete(prevGroupId));
}

/** Activate a Dockview group immediately; graph-session suspension remains serialized. */
export function focusEditorGroupSync(groupId: string): boolean {
  const previousGroupId = useGraphSessionStore.getState().getFocusedGroupId();
  const group = editorDockviewPort.listGroups().find((candidate) => candidate.groupId === groupId);
  if (!group) return false;
  if (editorDockviewPort.getActiveGroupId() !== groupId && group.activePanelInstanceId) {
    void editorDockviewPort.activate(group.activePanelInstanceId);
  }
  if (previousGroupId && previousGroupId !== groupId) scheduleSuspendPreviousGroup(previousGroupId);
  return previousGroupId !== groupId;
}

export async function awaitEditorGroupSessionChain(): Promise<void> {
  await editorGroupSessionChain;
}

export async function hydrateEditorGroup(groupId: string): Promise<boolean> {
  await editorGroupSessionChain;
  return activateCurrentEditorTab(groupId);
}

async function synchronizeTabSession(
  request: number,
  groupId: string,
  tab: LayoutTab,
): Promise<boolean> {
  if (tab.type === 'event' || tab.type === 'function') {
    focusDetails({ kind: tab.type, path: tab.id });
    const loaded = await activateGraphTab(tab.id, groupId);
    if (!loaded || request !== latestTabSwitchRequest) return false;
    syncVariablesGraphScopeFromActiveTab();
    return true;
  }

  if (tab.type === 'worksheet') {
    focusDetails({ kind: 'worksheet', worksheetPath: tab.id });
    const sessionStore = useGraphSessionStore.getState();
    if (sessionStore.getFocusedGroupId() === groupId) sessionStore.clearFocusedSession(groupId);
  }
  return true;
}

/** Synchronize a user-originated Dockview activation without writing back to Dockview. */
export async function synchronizeActiveEditorTab(groupId: string, tab: LayoutTab): Promise<boolean> {
  const request = ++latestTabSwitchRequest;
  focusEditorGroupSync(groupId);
  await editorGroupSessionChain;
  if (request !== latestTabSwitchRequest) return false;
  return synchronizeTabSession(request, groupId, tab);
}

/** Activate a tab requested by application code, then synchronize its business session. */
export async function switchEditorTab(groupId: string, tab: LayoutTab): Promise<boolean> {
  const request = ++latestTabSwitchRequest;
  focusEditorGroupSync(groupId);
  await editorGroupSessionChain;
  if (request !== latestTabSwitchRequest) return false;

  const panel = editorDockviewPort
    .findPanelsByResource(tab.id)
    .find((candidate) => candidate.groupId === groupId);
  if (panel && !panel.active) await editorDockviewPort.activate(panel.panelInstanceId);
  if (request !== latestTabSwitchRequest) return false;
  return synchronizeTabSession(request, groupId, tab);
}

export async function activateCurrentEditorTab(groupId: string): Promise<boolean> {
  const active = getActiveLayoutTab(groupId);
  if (!active) {
    useGraphSessionStore.getState().clearFocusedSession(groupId);
    return false;
  }
  if (active.tab.type === 'event' || active.tab.type === 'function') {
    const loaded = await activateGraphTab(active.tab.id, groupId);
    if (!loaded) return false;
    syncVariablesGraphScopeFromActiveTab();
    return true;
  }
  return active.tab.type === 'worksheet';
}

export async function activateEditorGroup(groupId: string): Promise<boolean> {
  focusEditorGroupSync(groupId);
  return hydrateEditorGroup(groupId);
}
