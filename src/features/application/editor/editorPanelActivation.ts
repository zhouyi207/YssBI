import {
  workbenchLayoutRead,
  revealWorkbenchView,
  type WorkbenchEditorPanelInfo,
} from "@/modules/workbench/public";
import { useGraphSessionStore } from "@/features/core/graphSession/graphSessionStore";
import { focusGraphPanelSession } from "./graphPanelSession";
import { detailFocusForEditorResource, setDetailContext } from "./rightSidebarActions";

type ActiveEditorPanelTarget = Pick<
  WorkbenchEditorPanelInfo,
  "panelInstanceId" | "groupId" | "metadata"
>;

function currentActiveEditorPanel(panel: ActiveEditorPanelTarget): WorkbenchEditorPanelInfo | null {
  const current = workbenchLayoutRead.getActiveEditorPanel();
  if (
    current?.panelInstanceId !== panel.panelInstanceId ||
    current.groupId !== panel.groupId ||
    current.metadata.resourceRef !== panel.metadata.resourceRef ||
    current.metadata.resourceKind !== panel.metadata.resourceKind
  )
    return null;
  return current;
}

/** Model selection drives focus and Details; it never loads or unloads a graph. */
export function synchronizeActiveEditorPanel(panel: ActiveEditorPanelTarget): boolean {
  const current = currentActiveEditorPanel(panel);
  if (!current) return false;
  const { metadata, groupId } = current;
  setDetailContext(
    detailFocusForEditorResource(
      metadata.resourceKind,
      metadata.resourceRef,
      panel.panelInstanceId,
    ),
  );
  if (metadata.resourceKind === "event_graph" || metadata.resourceKind === "function_graph")
    focusGraphPanelSession(metadata.resourceRef, groupId);
  else {
    const sessions = useGraphSessionStore.getState();
    const focusedGroup = sessions.getFocusedGroupId();
    if (focusedGroup) sessions.clearFocusedSession(focusedGroup);
  }
  return true;
}

/** Native open/reuse already selected the panel; never reactivate a late open result. */
export async function revealActiveEditorDetails(panel: WorkbenchEditorPanelInfo): Promise<boolean> {
  if (!synchronizeActiveEditorPanel(panel)) return false;
  if (workbenchLayoutRead.isReady) await revealWorkbenchView("details");
  return currentActiveEditorPanel(panel) !== null;
}

export function synchronizeCurrentEditorPanel(groupId: string): boolean {
  const active = workbenchLayoutRead.getActiveEditorPanelInGroup(groupId);
  if (!active) return false;
  return synchronizeActiveEditorPanel(active);
}
