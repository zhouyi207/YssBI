import {
  workbenchLayoutControl,
  workbenchLayoutRead,
  type WorkbenchEditorPanelInfo,
} from "@/modules/workbench/public";
import { useGraphSessionStore } from "@/features/core/graphSession/graphSessionStore";
import { focusGraphPanelSession } from "./graphPanelSession";
import { detailFocusForEditorResource, setPassiveDetailContext } from "./rightSidebarActions";

let latestPanelActivationRequest = 0;
type ActiveEditorPanelTarget = Pick<
  WorkbenchEditorPanelInfo,
  "panelInstanceId" | "groupId" | "metadata"
>;

/** Model selection drives focus and Details; it never loads or unloads a graph. */
export function synchronizeActiveEditorPanel(panel: ActiveEditorPanelTarget): boolean {
  const current = workbenchLayoutRead.getPanel(panel.panelInstanceId);
  if (
    current?.metadata.role !== "editor" ||
    current.groupId !== panel.groupId ||
    current.metadata.resourceRef !== panel.metadata.resourceRef
  )
    return false;
  const { metadata, groupId } = current;
  setPassiveDetailContext(
    detailFocusForEditorResource(metadata.resourceKind, metadata.resourceRef),
  );
  if (metadata.resourceKind === "event" || metadata.resourceKind === "function")
    focusGraphPanelSession(metadata.resourceRef, groupId);
  else {
    const sessions = useGraphSessionStore.getState();
    const focusedGroup = sessions.getFocusedGroupId();
    if (focusedGroup) sessions.clearFocusedSession(focusedGroup);
  }
  return true;
}

/** Application requests physical selection; all focus updates use the same idempotent coordinator. */
export async function activateEditorPanelAndSyncSession(
  panel: WorkbenchEditorPanelInfo,
): Promise<boolean> {
  const request = ++latestPanelActivationRequest;
  await Promise.resolve();
  if (request !== latestPanelActivationRequest) return false;
  const current = workbenchLayoutRead.getPanel(panel.panelInstanceId);
  if (current?.metadata.role !== "editor" || current.groupId !== panel.groupId) return false;
  if (
    workbenchLayoutRead.getActivePanel()?.panelInstanceId !== panel.panelInstanceId &&
    !(await workbenchLayoutControl.activate(panel.panelInstanceId))
  )
    return false;
  if (request !== latestPanelActivationRequest) return false;
  return synchronizeActiveEditorPanel({ ...current, metadata: current.metadata });
}

export function activateCurrentEditorPanel(groupId: string): boolean {
  const active = workbenchLayoutRead.getActiveEditorPanelInGroup(groupId);
  if (!active) return false;
  return synchronizeActiveEditorPanel(active);
}
