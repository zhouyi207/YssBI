import { workbenchLayoutRead } from "@/modules/workbench/public";
import { useEditorStore } from "@/features/core/editor/stores/useEditorStore";
import { isResourceDocumentDirty } from "@/features/core/resource";

import { requestCloseWorkbenchPanels } from "./workbenchPanelClose";
import { detailFocusForEditorResource, setDetailContext } from "./rightSidebarActions";

function editorPanelsInGroup(groupId: string) {
  return workbenchLayoutRead.listEditorPanelsInGroup(groupId);
}

function applyPassiveCloseFallback(): void {
  if (useEditorStore.getState().detailFocus) return;
  const active = workbenchLayoutRead.getActiveEditorPanel();
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
