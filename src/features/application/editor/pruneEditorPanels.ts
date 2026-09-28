import {
  commitWorkbenchPanelRemoval,
  releaseEditorPaneState,
  workbenchLayoutRead,
  type WorkbenchPanelCommitToken,
} from "@/modules/workbench/public";
import { shouldRetainResourceEditor } from "@/features/core/resource";
import {
  captureProjectLifecycleState,
  isProjectLifecycleStateCurrent,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";

/** Missing resources retain their editor until unfinished work is saved or discarded. */
export async function pruneEditorPanelsForMissingResources(): Promise<void> {
  const identity = captureProjectLifecycleState();
  const stalePanels = workbenchLayoutRead.listPanels().filter((panel) => {
    if (panel.metadata.role !== "editor") return false;
    return !shouldRetainResourceEditor({
      id: panel.metadata.resourceRef,
      kind: panel.metadata.resourceKind,
    });
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
          !shouldRetainResourceEditor({ id: metadata.resourceRef, kind: metadata.resourceKind }),
      ),
  );
  if (outcome !== "committed") return;

  for (const panel of stalePanels) releaseEditorPaneState(panel.panelInstanceId);
}
