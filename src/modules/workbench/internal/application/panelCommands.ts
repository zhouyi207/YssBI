import { useEditorPaneStateStore } from "../layout/editorPaneStateStore";
import { workbenchLayoutInternal } from "../layout/workbenchLayoutInternal";
import { workbenchLayoutRead } from "../layout/workbenchRead";
import type { EditorResourceKind } from "../layout/workbenchPanelModel";
import type { WorkbenchPanelCommitToken } from "../layout/workbenchTypes";

export interface EditorPanelResourceMove {
  readonly from: string;
  readonly to: string;
}

export function commitWorkbenchPanelRemoval(
  tokens: readonly WorkbenchPanelCommitToken[],
  isCurrent?: () => boolean,
): Promise<"committed" | "stale"> {
  return workbenchLayoutInternal.commitRemove(tokens, isCurrent);
}

export function releaseEditorPaneState(panelInstanceId: string): void {
  useEditorPaneStateStore.getState().release(panelInstanceId);
}

export function resetEditorPaneState(): void {
  useEditorPaneStateStore.getState().reset();
}

export async function closeWorkbenchViewPanel(panelInstanceId: string): Promise<boolean> {
  const panel = workbenchLayoutRead.getPanel(panelInstanceId);
  if (!panel || panel.metadata.role !== "view") return false;
  const outcome = await commitWorkbenchPanelRemoval([
    {
      panelInstanceId: panel.panelInstanceId,
      groupId: panel.groupId,
      metadata: structuredClone(panel.metadata),
    },
  ]);
  return outcome === "committed";
}

export async function removeProjectScopedPanelsFromWorkbench(
  isCurrent: () => boolean,
): Promise<void> {
  if (!workbenchLayoutRead.isReady || !isCurrent()) return;
  await workbenchLayoutInternal.runLayoutTransaction((transaction) => {
    if (!isCurrent()) return;
    const panelInstanceIds = transaction
      .listPanels()
      .filter((panel) => panel.metadata.role === "editor" || panel.metadata.role === "result")
      .map((panel) => panel.panelInstanceId);
    if (isCurrent()) transaction.removePanels(panelInstanceIds);
  });
}

export function commitEditorPanelPublication(
  moves: Iterable<EditorPanelResourceMove>,
  isResourceAvailable: (resourceKind: EditorResourceKind, resourceRef: string) => boolean,
  commitBusinessStores: () => void,
  isCurrent: () => boolean = () => true,
): void | Promise<void> {
  if (!isCurrent()) return;
  if (!workbenchLayoutRead.isReady) {
    commitBusinessStores();
    return;
  }
  return commitEditorPanelPublicationWithFlexLayout(
    [...moves],
    isResourceAvailable,
    commitBusinessStores,
    isCurrent,
  );
}

async function commitEditorPanelPublicationWithFlexLayout(
  moves: readonly EditorPanelResourceMove[],
  isResourceAvailable: (resourceKind: EditorResourceKind, resourceRef: string) => boolean,
  commitBusinessStores: () => void,
  isCurrent: () => boolean,
): Promise<void> {
  const removedPanelIds = await workbenchLayoutInternal.runPublicationTransaction((transaction) => {
    if (!isCurrent()) return [];
    for (const move of moves) transaction.remapResource(move.from, move.to);

    const removed = transaction.listPanels().flatMap((panel) => {
      if (panel.metadata.role !== "editor") return [];
      return isResourceAvailable(panel.metadata.resourceKind, panel.metadata.resourceRef)
        ? []
        : [panel.panelInstanceId];
    });
    transaction.removePanels(removed);
    commitBusinessStores();
    return removed;
  });

  for (const panelInstanceId of removedPanelIds) releaseEditorPaneState(panelInstanceId);
}
