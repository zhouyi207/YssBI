import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import {
  ProjectLifecycleProtocolError,
  type ProjectLifecycleReceiptDependencies,
} from '@/features/application/projectLifecycleReceipt';
import {
  commitPreparedAuthoritativeProjectLoad,
  prepareAuthoritativeProjectLoad,
  useProjectIOStore,
} from '@/features/core/dataStore/projectIOStore';
import { ProjectService } from '@/services/project/projectService';
import {
  captureProjectIdentity,
  isProjectLifecycleStateCurrent,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import {
  revokeAllPinPreviewLeases,
  useExecutionStore,
} from '@/features/core/execution';
import { clearCanvasInteractionProject } from '@/features/core/canvas/canvasInteractionCleanup';

export function createProjectLifecycleReceiptDependencies(): ProjectLifecycleReceiptDependencies {
  return {
    prepareProjectTransition: async () => {
      const prepared = await prepareAuthoritativeProjectLoad(captureProjectIdentity());
      return {
        projectInstanceId: prepared.index.projectInstanceId,
        publicationRevision: prepared.index.publicationRevision,
        commit: async () => {
          await commitPreparedAuthoritativeProjectLoad(prepared);
        },
      };
    },
    refreshRegistry: () => ProjectService.listRegisteredProjects(),
    clearProject: async (owner) => {
      if (!isProjectLifecycleStateCurrent(owner)) return;
      clearCanvasInteractionProject();
      if (!isProjectLifecycleStateCurrent(owner)) return;
      revokeAllPinPreviewLeases();
      if (!isProjectLifecycleStateCurrent(owner)) return;
      useExecutionStore.setState({
        graphs: {},
        playbackGraphPath: null,
        isPlaying: false,
      });
      if (!isProjectLifecycleStateCurrent(owner)) return;
      await useProjectIOStore.getState().loadProjectFromData(
        {
          variables: {},
          graphs: {},
          databases: {},
          metadata: { exportTime: '' },
        },
        null,
        owner,
      );
      if (isProjectLifecycleStateCurrent(owner)
        && useProjectIOStore.getState().projectInstanceId !== owner.projectInstanceId) {
        throw new ProjectLifecycleProtocolError('Project clear retained an unexpected store identity');
      }
    },
    markProjectStale: () => projectPublicationCoordinator.markProjectProjectionStale(),
  };
}
