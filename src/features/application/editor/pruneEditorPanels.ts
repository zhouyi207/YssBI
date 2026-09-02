import {
  commitWorkbenchPanelRemoval,
  releaseEditorPaneState,
  workbenchDockviewRead,
  type WorkbenchPanelCommitToken,
} from "@/modules/workbench/public";
import { resourceKey, useResourceStore } from "@/features/core/resource";

/** Atomically remove restored editors whose project resources are absent after hydration. */
export async function pruneEditorPanelsForMissingResources(): Promise<void> {
  const resources = useResourceStore.getState().resources;
  const stalePanels = workbenchDockviewRead.listPanels().filter((panel) => {
    if (panel.metadata.role !== "editor") return false;
    return !resources[
      resourceKey({
        id: panel.metadata.resourceRef,
        kind: panel.metadata.resourceKind,
      })
    ];
  });
  if (stalePanels.length === 0) return;

  const tokens: WorkbenchPanelCommitToken[] = stalePanels.map((panel) => ({
    panelInstanceId: panel.panelInstanceId,
    groupId: panel.groupId,
    metadata: structuredClone(panel.metadata),
  }));
  const outcome = await commitWorkbenchPanelRemoval(tokens);
  if (outcome !== "committed") return;

  for (const panel of stalePanels) releaseEditorPaneState(panel.panelInstanceId);
}
