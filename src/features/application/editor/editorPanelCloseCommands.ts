import { workbenchDockviewRead } from "@/features/core/dockview/workbenchRead";
import { useEditorStore } from "@/features/core/editor/stores/useEditorStore";
import { isResourceDocumentDirty } from "@/features/core/resource";

import { splitEditorPanel } from "./editorGroupCommands";
import { requestCloseWorkbenchPanels } from "./workbenchPanelClose";
import { detailFocusForEditorResource, setDetailContext } from "./rightSidebarActions";

function editorPanelsInGroup(groupId: string) {
  return workbenchDockviewRead.listEditorPanelsInGroup(groupId);
}

function applyPassiveCloseFallback(): void {
  if (useEditorStore.getState().detailFocus) return;
  const active = workbenchDockviewRead.getActiveEditorPanel();
  if (active?.metadata.role !== "editor") return;

  const { resourceKind, resourceRef } = active.metadata;
  setDetailContext(detailFocusForEditorResource(resourceKind, resourceRef));
}

async function requestClosePanelsAndApplyFallback(
  panelInstanceIds: readonly string[],
): Promise<boolean> {
  if (panelInstanceIds.length === 0) return true;
  const closed = await requestCloseWorkbenchPanels(panelInstanceIds);
  if (closed) applyPassiveCloseFallback();
  return closed;
}

export function requestCloseEditorPanels(panelInstanceIds: readonly string[]): Promise<boolean> {
  return requestClosePanelsAndApplyFallback(panelInstanceIds);
}

export function requestCloseEditorPanel(panelInstanceId: string): Promise<boolean> {
  return requestCloseEditorPanels([panelInstanceId]);
}

export function requestCloseOtherEditorPanels(
  groupId: string,
  keepPanelInstanceId: string,
): Promise<boolean> {
  return requestCloseEditorPanels(
    editorPanelsInGroup(groupId)
      .filter((panel) => panel.panelInstanceId !== keepPanelInstanceId)
      .map((panel) => panel.panelInstanceId),
  );
}

export function requestCloseAllEditorPanelsInGroup(groupId: string): Promise<boolean> {
  return requestCloseEditorPanels(
    editorPanelsInGroup(groupId).map((panel) => panel.panelInstanceId),
  );
}

export function requestCloseSavedEditorPanelsInGroup(groupId: string): Promise<boolean> {
  return requestCloseEditorPanels(
    editorPanelsInGroup(groupId)
      .filter(
        (panel) =>
          !isResourceDocumentDirty({
            id: panel.metadata.resourceRef,
            kind: panel.metadata.resourceKind,
          }),
      )
      .map((panel) => panel.panelInstanceId),
  );
}

/** Physical Close Group owns every canonical panel currently in that Dockview group. */
export function closeEditorGroup(groupId: string): Promise<boolean> {
  return requestClosePanelsAndApplyFallback(
    workbenchDockviewRead.listGroupPanels(groupId).map((panel) => panel.panelInstanceId),
  );
}

export async function splitEditorGroup(
  groupId: string,
  direction: "row" | "col" = "row",
): Promise<void> {
  await splitEditorPanel(groupId, direction === "row" ? "right" : "bottom");
}
