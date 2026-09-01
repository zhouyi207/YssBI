import { workbenchDockviewControl } from "@/features/core/dockview/workbenchControl";
import {
  workbenchDockviewRead,
  type WorkbenchEditorPanelInfo,
} from "@/features/core/dockview/workbenchRead";
import { syncVariablesGraphScopeFromActiveTab } from "@/features/core/editor/detail/variablesGraphScope";
import { useGraphSessionStore } from "@/features/core/graphSession/graphSessionStore";

import { activateGraphPanelSession } from "./graphPanelSession";
import { suspendEditorGroupGraphSession } from "./graphSessionLifecycle";
import { detailFocusForEditorResource, setPassiveDetailContext } from "./rightSidebarActions";

let editorGroupSessionChain: Promise<void> = Promise.resolve();
let latestPanelActivationRequest = 0;
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
  return workbenchDockviewRead.listEditorPanelsInGroup(groupId).length > 0;
}

/** Synchronize application session focus without writing layout focus back to Dockview. */
export function focusEditorGroupSync(groupId: string): boolean {
  const groupExists = workbenchDockviewRead.listGroups().some((group) => group.groupId === groupId);
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
  return activateCurrentEditorPanel(groupId);
}

async function synchronizePanelSession(
  request: number,
  panel: WorkbenchEditorPanelInfo,
): Promise<boolean> {
  const { groupId, metadata } = panel;
  if (metadata.resourceKind === "event" || metadata.resourceKind === "function") {
    setPassiveDetailContext(
      detailFocusForEditorResource(metadata.resourceKind, metadata.resourceRef),
    );
    const loaded = await activateGraphPanelSession(metadata.resourceRef, groupId);
    if (!loaded || request !== latestPanelActivationRequest) return false;
    syncVariablesGraphScopeFromActiveTab();
    return true;
  }

  if (metadata.resourceKind === "worksheet") {
    setPassiveDetailContext(
      detailFocusForEditorResource(metadata.resourceKind, metadata.resourceRef),
    );
    const sessionStore = useGraphSessionStore.getState();
    if (sessionStore.getFocusedGroupId() === groupId) {
      sessionStore.clearFocusedSession(groupId);
    }
    return true;
  }
  return false;
}

/** Synchronize a user-originated Dockview activation without writing back to Dockview. */
export async function synchronizeActiveEditorPanel(
  panel: WorkbenchEditorPanelInfo,
): Promise<boolean> {
  const request = ++latestPanelActivationRequest;
  focusEditorGroupSync(panel.groupId);
  await editorGroupSessionChain;
  if (request !== latestPanelActivationRequest) return false;
  return synchronizePanelSession(request, panel);
}

/** Activate an application-requested editor, then synchronize its business session. */
export async function activateEditorPanelAndSyncSession(
  panel: WorkbenchEditorPanelInfo,
): Promise<boolean> {
  const request = ++latestPanelActivationRequest;
  focusEditorGroupSync(panel.groupId);
  await editorGroupSessionChain;
  if (request !== latestPanelActivationRequest) return false;

  const current = workbenchDockviewRead.getPanel(panel.panelInstanceId);
  if (current?.metadata.role !== "editor" || current.groupId !== panel.groupId) return false;
  if (
    workbenchDockviewRead.getActivePanel()?.panelInstanceId !== panel.panelInstanceId &&
    !(await workbenchDockviewControl.activate(panel.panelInstanceId))
  )
    return false;
  if (request !== latestPanelActivationRequest) return false;
  return synchronizePanelSession(request, { ...current, metadata: current.metadata });
}

export async function activateCurrentEditorPanel(groupId: string): Promise<boolean> {
  const active = workbenchDockviewRead.getActiveEditorPanelInGroup(groupId);
  if (!active) {
    useGraphSessionStore.getState().clearFocusedSession(groupId);
    return false;
  }
  const request = ++latestPanelActivationRequest;
  return synchronizePanelSession(request, active);
}

export async function activateEditorGroup(groupId: string): Promise<boolean> {
  const active = workbenchDockviewRead.getActiveEditorPanelInGroup(groupId);
  return active ? activateEditorPanelAndSyncSession(active) : false;
}
