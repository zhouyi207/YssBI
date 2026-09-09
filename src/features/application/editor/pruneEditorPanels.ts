import {
  commitWorkbenchPanelRemoval,
  releaseEditorPaneState,
  workbenchDockviewRead,
  type WorkbenchPanelCommitToken,
} from "@/modules/workbench/public";
import { resourceKey, useResourceStore } from "@/features/core/resource";
import {
  captureProjectLifecycleState,
  isProjectLifecycleStateCurrent,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";

/** Remove editors for absent resources, including retained loaded-but-missing records. */
export async function pruneEditorPanelsForMissingResources(): Promise<void> {
  const identity = captureProjectLifecycleState();
  const resources = useResourceStore.getState().resources;
  const stalePanels = workbenchDockviewRead.listPanels().filter((panel) => {
    if (panel.metadata.role !== "editor") return false;
    return !resources[
      resourceKey({
        id: panel.metadata.resourceRef,
        kind: panel.metadata.resourceKind,
      })
    ]?.exists;
  });
  if (stalePanels.length === 0) return;

  const tokens: WorkbenchPanelCommitToken[] = stalePanels.map((panel) => ({
    panelInstanceId: panel.panelInstanceId,
    groupId: panel.groupId,
    metadata: structuredClone(panel.metadata),
  }));
  const outcome = await commitWorkbenchPanelRemoval(
    tokens,
    () =>
      isProjectLifecycleStateCurrent(identity) &&
      tokens.every(
        ({ metadata }) =>
          metadata.role === "editor" &&
          !useResourceStore.getState().resources[
            resourceKey({ id: metadata.resourceRef, kind: metadata.resourceKind })
          ]?.exists,
      ),
  );
  if (outcome !== "committed") return;

  for (const panel of stalePanels) releaseEditorPaneState(panel.panelInstanceId);
}
