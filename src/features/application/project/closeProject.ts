import { i18n } from "@/app/i18n";
import { ProjectService } from "@/services/project/projectService";
import {
  captureProjectLifecycleState,
  isProjectLifecycleStateCurrent,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";
import { createProjectLifecycleReceiptDependencies } from "@/features/application/projectLifecycleReceiptDependencies";
import { resetResultQueryProject } from "@/features/application/results";
import { confirmDirtyEditorClose } from "@/features/application/editor/confirmDirtyEditorClose";
import { showBlockingIpcError } from "@/features/application/editor/blockingErrorDialog";
import { workbenchLayoutController } from "@/modules/workbench/public";
import { useProjectIOStore } from "./projectIOStore";

let closing: Promise<boolean> | null = null;
let clearing: { projectInstanceId: string; promise: Promise<void> } | null = null;

/** Command completion and its event share one clear; late events cannot clear a new project. */
export function applyProjectClosed(projectInstanceId: string): Promise<void> {
  if (clearing?.projectInstanceId === projectInstanceId) return clearing.promise;
  const current = captureProjectLifecycleState();
  if (current.projectInstanceId !== projectInstanceId) return Promise.resolve();

  projectPublicationCoordinator.cancelProject();
  resetResultQueryProject();
  const owner = captureProjectLifecycleState();
  const entry = {
    projectInstanceId,
    promise: createProjectLifecycleReceiptDependencies().clearProject(owner),
  };
  clearing = entry;
  return entry.promise.finally(() => {
    if (clearing === entry) clearing = null;
  });
}

async function closeCurrentProject(): Promise<boolean> {
  try {
    const owner = captureProjectLifecycleState();
    if (!(await confirmDirtyEditorClose()) || !isProjectLifecycleStateCurrent(owner)) return false;
    await workbenchLayoutController.flushBeforeWindowClose();
    if (!isProjectLifecycleStateCurrent(owner)) return false;
    if (owner.projectInstanceId) {
      await ProjectService.closeProject(owner.projectInstanceId);
      await applyProjectClosed(owner.projectInstanceId);
    }
    const current = captureProjectLifecycleState();
    const store = useProjectIOStore.getState();
    return (
      current.projectInstanceId === null &&
      store.projectInstanceId === null &&
      store.currentPath === null
    );
  } catch (error) {
    showBlockingIpcError(error, "close_project", (code) =>
      i18n.t("notifications.project.closeFailed", { error: code }),
    );
    return false;
  }
}

/** Navigate to the picker only after this returns true. Repeated requests share one decision. */
export function requestCloseProject(): Promise<boolean> {
  if (!closing) {
    closing = closeCurrentProject().finally(() => {
      closing = null;
    });
  }
  return closing;
}
