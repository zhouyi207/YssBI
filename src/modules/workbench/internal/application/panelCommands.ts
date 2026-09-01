import { useEditorPaneStateStore } from "../dockview/editorPaneStateStore";
import { workbenchDockviewInternal } from "../dockview/workbenchDockviewInternal";
import { workbenchDockviewRead } from "../dockview/workbenchRead";
import type { EditorResourceKind } from "../dockview/workbenchPanelModel";
import type { WorkbenchPanelCommitToken } from "../dockview/workbenchTypes";

export interface EditorPanelResourceMove {
  readonly from: string;
  readonly to: string;
}

export function commitWorkbenchPanelRemoval(
  tokens: readonly WorkbenchPanelCommitToken[],
  isCurrent?: () => boolean,
): Promise<"committed" | "stale"> {
  return workbenchDockviewInternal.commitRemove(tokens, isCurrent);
}

export function releaseEditorPaneState(panelInstanceId: string): void {
  useEditorPaneStateStore.getState().release(panelInstanceId);
}

export function resetEditorPaneState(): void {
  useEditorPaneStateStore.getState().reset();
}

export async function closeWorkbenchViewPanel(panelInstanceId: string): Promise<boolean> {
  const panel = workbenchDockviewRead.getPanel(panelInstanceId);
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
  if (!workbenchDockviewRead.isReady || !isCurrent()) return;
  await workbenchDockviewInternal.runLayoutTransaction((transaction) => {
    if (!isCurrent()) return;
    const panelInstanceIds = transaction
      .listPanels()
      .filter(
        (panel) =>
          panel.metadata.role === "editor" ||
          panel.metadata.role === "result" ||
          (panel.metadata.role === "view" && panel.metadata.viewId === "inspect"),
      )
      .map((panel) => panel.panelInstanceId);
    if (isCurrent()) transaction.removePanels(panelInstanceIds);
  });
}

export function commitEditorPanelPublication(
  moves: Iterable<EditorPanelResourceMove>,
  isResourceAvailable: (resourceKind: EditorResourceKind, resourceRef: string) => boolean,
  commitBusinessStores: () => void,
): void | Promise<void> {
  if (!workbenchDockviewRead.isReady) {
    commitBusinessStores();
    return;
  }
  return commitEditorPanelPublicationWithDockview(
    [...moves],
    isResourceAvailable,
    commitBusinessStores,
  );
}

async function commitEditorPanelPublicationWithDockview(
  moves: readonly EditorPanelResourceMove[],
  isResourceAvailable: (resourceKind: EditorResourceKind, resourceRef: string) => boolean,
  commitBusinessStores: () => void,
): Promise<void> {
  const removedPanelIds = await workbenchDockviewInternal.runPublicationTransaction(
    (transaction) => {
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
    },
  );

  for (const panelInstanceId of removedPanelIds) releaseEditorPaneState(panelInstanceId);
}
