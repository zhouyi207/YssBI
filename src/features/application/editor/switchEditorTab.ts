import { workbenchDockviewPort } from '@/features/core/dockview/workbenchDockviewPort';
import { syncVariablesGraphScopeFromActiveTab } from '@/features/core/editor/detail/variablesGraphScope';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { getActiveLayoutTab } from '@/features/core/layout/layoutTabQueries';
import type { LayoutTab } from '@/shared/types/ui';

import { activateGraphTab } from './activateGraphTab';
import { suspendEditorGroupGraphSession } from './graphSessionLifecycle';
import {
  detailFocusForEditorResource,
  setPassiveDetailContext,
} from './rightSidebarActions';

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

function groupContainsEditor(groupId: string): boolean {
  return workbenchDockviewPort
    .listGroupPanels(groupId)
    .some((panel) => panel.metadata.role === 'editor');
}

/** Synchronize application session focus without writing layout focus back to Dockview. */
export function focusEditorGroupSync(groupId: string): boolean {
  const groupExists = workbenchDockviewPort
    .listGroups()
    .some((group) => group.groupId === groupId);
  if (!groupExists || !groupContainsEditor(groupId)) return false;

  const previousGroupId = useGraphSessionStore.getState().getFocusedGroupId();
  if (previousGroupId && previousGroupId !== groupId) {
    scheduleSuspendPreviousGroup(previousGroupId);
  }
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
    setPassiveDetailContext(detailFocusForEditorResource(tab.type, tab.id));
    const loaded = await activateGraphTab(tab.id, groupId);
    if (!loaded || request !== latestTabSwitchRequest) return false;
    syncVariablesGraphScopeFromActiveTab();
    return true;
  }

  if (tab.type === 'worksheet') {
    setPassiveDetailContext(detailFocusForEditorResource(tab.type, tab.id));
    const sessionStore = useGraphSessionStore.getState();
    if (sessionStore.getFocusedGroupId() === groupId) {
      sessionStore.clearFocusedSession(groupId);
    }
    return true;
  }
  return false;
}

/** Synchronize a user-originated Dockview activation without writing back to Dockview. */
export async function synchronizeActiveEditorTab(
  groupId: string,
  tab: LayoutTab,
): Promise<boolean> {
  const request = ++latestTabSwitchRequest;
  focusEditorGroupSync(groupId);
  await editorGroupSessionChain;
  if (request !== latestTabSwitchRequest) return false;
  return synchronizeTabSession(request, groupId, tab);
}

/** Activate an application-requested editor, then synchronize its business session. */
export async function switchEditorTab(groupId: string, tab: LayoutTab): Promise<boolean> {
  const request = ++latestTabSwitchRequest;
  focusEditorGroupSync(groupId);
  await editorGroupSessionChain;
  if (request !== latestTabSwitchRequest) return false;

  const panel = workbenchDockviewPort
    .findEditorPanelsByResource(tab.id)
    .find((candidate) =>
      candidate.groupId === groupId
        && candidate.metadata.role === 'editor'
        && candidate.metadata.resourceKind === tab.type);
  if (!panel) return false;
  if (!panel.active && !await workbenchDockviewPort.activate(panel.panelInstanceId)) return false;
  if (request !== latestTabSwitchRequest) return false;
  return synchronizeTabSession(request, groupId, tab);
}

export async function activateCurrentEditorTab(groupId: string): Promise<boolean> {
  const active = getActiveLayoutTab(groupId);
  if (!active) {
    useGraphSessionStore.getState().clearFocusedSession(groupId);
    return false;
  }
  const request = ++latestTabSwitchRequest;
  return synchronizeTabSession(request, groupId, active.tab);
}

export async function activateEditorGroup(groupId: string): Promise<boolean> {
  const active = getActiveLayoutTab(groupId);
  return active ? switchEditorTab(groupId, active.tab) : false;
}
