import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import type { ProjectLifecycleReceiptDependencies } from '@/features/application/projectLifecycleReceipt';
import {
  commitPreparedAuthoritativeProjectLoad,
  prepareAuthoritativeProjectLoad,
  useProjectIOStore,
} from '@/features/core/dataStore/projectIOStore';
import { ProjectService } from '@/services/project/projectService';
import { captureProjectIdentity } from '@/services/project/projectIdentity';

export function createProjectLifecycleReceiptDependencies(
  onProjectCleared?: () => void,
): ProjectLifecycleReceiptDependencies {
  return {
    prepareProjectTransition: async () => {
      const prepared = await prepareAuthoritativeProjectLoad(captureProjectIdentity());
      return {
        projectInstanceId: prepared.index.projectInstanceId,
        publicationRevision: prepared.index.publicationRevision,
        commit: () => {
          commitPreparedAuthoritativeProjectLoad(prepared);
        },
      };
    },
    refreshRegistry: () => ProjectService.listRegisteredProjects(),
    clearProject: () => {
      projectPublicationCoordinator.cancelProject();
      useProjectIOStore.getState().loadProjectFromData(
        {
          variables: {},
          graphs: {},
          databases: {},
          metadata: { exportTime: '', appVersion: '' },
        },
        null,
      );
      useProjectIOStore.setState({ projectInstanceId: null });
      onProjectCleared?.();
    },
    markProjectStale: () => projectPublicationCoordinator.markProjectProjectionStale(),
  };
}
