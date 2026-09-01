import { useEditorPaneStateStore } from "@/features/core/dockview/editorPaneStateStore";
import { workbenchDockviewInternal } from "@/features/core/dockview/workbenchDockviewInternal";
import { workbenchDockviewRead } from "@/features/core/dockview/workbenchRead";
import type { WorkbenchPanelCommitToken } from "@/features/core/dockview/workbenchTypes";
import { resourceKey, useResourceStore } from "@/features/core/resource";

/** Atomically remove restored editors whose project resources are absent after hydration. */
export async function reconcileOpenEditorPanelsWithResources(): Promise<void> {
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
  const outcome = await workbenchDockviewInternal.commitRemove(tokens);
  if (outcome !== "committed") return;

  const paneState = useEditorPaneStateStore.getState();
  for (const panel of stalePanels) paneState.release(panel.panelInstanceId);
}
