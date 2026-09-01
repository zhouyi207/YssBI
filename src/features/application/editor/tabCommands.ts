import { workbenchDockviewRead } from "@/features/core/dockview/workbenchRead";
import { useEditorStore } from "@/features/core/editor/stores/useEditorStore";
import { isResourceDocumentDirty } from "@/features/core/resource";

import { splitEditorAtEdge } from "./editorGroupCommands";
import { requestCloseWorkbenchPanels } from "./workbenchPanelClose";
import { detailFocusForEditorResource, setDetailContext } from "./rightSidebarActions";

function editorPanelsInGroup(groupId: string) {
  return workbenchDockviewRead
    .listGroupPanels(groupId)
    .filter((panel) => panel.metadata.role === "editor");
}

function applyPassiveCloseFallback(): void {
  if (useEditorStore.getState().detailFocus) return;
  const active = workbenchDockviewRead.getActiveEditorPanel();
  if (active?.metadata.role !== "editor") return;

  const { resourceKind, resourceRef } = active.metadata;
  setDetailContext(detailFocusForEditorResource(resourceKind, resourceRef));
}

async function closePanelIds(panelInstanceIds: readonly string[]): Promise<boolean> {
  if (panelInstanceIds.length === 0) return true;
  const closed = await requestCloseWorkbenchPanels(panelInstanceIds);
  if (closed) applyPassiveCloseFallback();
  return closed;
}

export function closeTab(panelInstanceId: string): Promise<boolean> {
  return closePanelIds([panelInstanceId]);
}

export function closeOtherTabs(groupId: string, keepPanelInstanceId: string): Promise<boolean> {
  return closePanelIds(
    editorPanelsInGroup(groupId)
      .filter((panel) => panel.panelInstanceId !== keepPanelInstanceId)
      .map((panel) => panel.panelInstanceId),
  );
}

export function closeAllTabsInGroup(groupId: string): Promise<boolean> {
  return closePanelIds(editorPanelsInGroup(groupId).map((panel) => panel.panelInstanceId));
}

export function closeSavedTabsInGroup(groupId: string): Promise<boolean> {
  return closePanelIds(
    editorPanelsInGroup(groupId)
      .filter(
        (panel) =>
          panel.metadata.role === "editor" &&
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
  return closePanelIds(
    workbenchDockviewRead.listGroupPanels(groupId).map((panel) => panel.panelInstanceId),
  );
}

export async function splitEditorGroup(
  groupId: string,
  direction: "row" | "col" = "row",
): Promise<void> {
  await splitEditorAtEdge(groupId, direction === "row" ? "right" : "bottom");
}
