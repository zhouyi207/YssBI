import { LoadStatus } from "@/shared/types/ui/common";
import { ProjectService } from "@/services/project/projectService";
import { captureProjectLifecycleState } from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import {
  loadActivatedProject,
  loadCurrentProject,
} from "@/features/application/project/projectHydration";
import { hydrateProjectPath } from "@/features/application/project/projectSession";

/** Hydrate the current backend project for a window through the Application entrance. */
export async function initializeProjectForCurrentWindow(): Promise<void> {
  if (!captureProjectLifecycleState().projectInstanceId) {
    await loadActivatedProject(await ProjectService.getProjectActivation());
    return;
  }

  const { status, currentPath } = useProjectIOStore.getState();
  if (status === LoadStatus.Ready) {
    if (currentPath) return;
    const hydrated = await hydrateProjectPath();
    if (!hydrated) return;
  }
  await loadCurrentProject();
}
