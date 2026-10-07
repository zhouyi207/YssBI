import { ProjectLifecycleProtocolError } from "@/features/application/projectLifecycleReceipt";
import { resetProjectScopedRightSidebarState } from "@/features/application/project/projectReset";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import { useGraphSessionStore } from "@/features/core/graphSession/graphSessionStore";
import {
  removeProjectScopedPanelsFromWorkbench,
  resetEditorPaneState,
  workbenchLayoutController,
} from "@/modules/workbench/public";
import {
  isProjectLifecycleStateCurrent,
  type ProjectLifecycleStateSnapshot,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";

function ownsPreviousProject(
  previousProjectInstanceId: string,
  owner: ProjectLifecycleStateSnapshot,
): boolean {
  if (!isProjectLifecycleStateCurrent(owner)) return false;
  if (useProjectIOStore.getState().projectInstanceId !== previousProjectInstanceId) {
    throw new ProjectLifecycleProtocolError("stale project cleanup", true);
  }
  return true;
}

export async function removeProjectScopedWorkbenchPanels(
  previousProjectInstanceId: string,
  owner: ProjectLifecycleStateSnapshot,
): Promise<void> {
  if (!ownsPreviousProject(previousProjectInstanceId, owner)) return;
  workbenchLayoutController.invalidateForProjectReplacement();
  if (!ownsPreviousProject(previousProjectInstanceId, owner)) return;

  await removeProjectScopedPanelsFromWorkbench(() =>
    ownsPreviousProject(previousProjectInstanceId, owner),
  );

  if (!ownsPreviousProject(previousProjectInstanceId, owner)) return;
  resetEditorPaneState();
  if (!ownsPreviousProject(previousProjectInstanceId, owner)) return;
  useGraphSessionStore.getState().reset();
  if (!ownsPreviousProject(previousProjectInstanceId, owner)) return;
  resetProjectScopedRightSidebarState();
  ownsPreviousProject(previousProjectInstanceId, owner);
}
