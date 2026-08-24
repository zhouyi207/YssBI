import { registerProjectIOApplicationPort } from '@/features/core/dataStore/projectIOApplicationPort';
import { registerSyncApplicationEventPort } from '@/features/core/sync/applicationEventPort';
import { registerPendingMutationPort } from '@/features/core/history/pendingMutationPort';
import { registerWorksheetApplicationPort } from '@/features/core/worksheet/worksheetApplicationPort';
import { hydrateFunctionSignaturesFromProjectIndex } from '@/features/application/graphDocument/functionSignatureSync';
import { resetFunctionSignatureCoordinator } from '@/features/application/editorMutation/functionSignatureCoordinator';
import { resetHistoryCoordinator } from '@/features/application/editorMutation/historyCoordinator';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import { getPendingMutation } from '@/features/application/editorMutation/pendingMutationRegistry';
import { bootstrapEditorGraphSession } from '@/features/application/editor/bootstrapEditorGraphSession';
import { reconcileOpenLayoutTabsWithResources } from '@/features/application/editor/reconcileOpenLayoutTabs';
import { workbenchLayoutController } from '@/features/application/layout/workbenchLayoutController';
import { removeProjectScopedWorkbenchPanels } from '@/features/application/project/projectWorkbenchLifecycle';
import { workbenchDockviewPort } from '@/features/core/dockview/workbenchDockviewPort';
import { captureProjectLifecycleState } from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import {
  beginGraphLoadLifecycle,
  invalidateGraphProjection,
  loadGraphProjection,
  resetGraphProjectionCoordinator,
} from '@/features/application/editorProjection/graphProjectionCoordinator';

import { applyProjectLifecycleReceipt } from '@/features/application/projectLifecycleReceipt';
import { createProjectLifecycleReceiptDependencies } from '@/features/application/projectLifecycleReceiptDependencies';
import { captureProjectCommandContext } from '@/features/application/projectCommandContext';
import { reconcileProjectComputationSettingsEvent } from '@/features/application/projectSettings/useProjectComputationSettings';

export function registerCoreApplicationPorts(): void {
  registerProjectIOApplicationPort({
    hydrateFunctionSignatures: hydrateFunctionSignaturesFromProjectIndex,
    resetFunctionSignatures: resetFunctionSignatureCoordinator,
    resetHistory: resetHistoryCoordinator,
    validatePublicationStart: (id, revision) => projectPublicationCoordinator.validateProjectStart(id, revision),
    startPublication: (id, revision) => projectPublicationCoordinator.startProject(id, revision),
    acceptProjectActivation: (id, revision) => projectPublicationCoordinator.acceptProjectActivation(id, revision),
    reconcileOpenTabs: () => {
      workbenchLayoutController.markProjectResourcesReady(async (context) => {
        if (!context.isCurrent()) return;
        await reconcileOpenLayoutTabsWithResources();
        if (!context.isCurrent()) return;
        const active = workbenchDockviewPort.getActiveEditorPanel();
        if (active?.metadata.role === 'editor') {
          await bootstrapEditorGraphSession(active.groupId);
        }
      });
    },
    removeProjectScopedWorkbenchPanels,
    resetGraphProjection: resetGraphProjectionCoordinator,
    beginGraphLoad: beginGraphLoadLifecycle,
    loadGraphProjection,
    submitPublication: async (result) => projectPublicationCoordinator.submit({ result: result as never }),
  });
  registerWorksheetApplicationPort({
    captureCommandContext: captureProjectCommandContext,
    submitPublication: async (result) => projectPublicationCoordinator.submit({ result }),
  });
  registerPendingMutationPort({
    graphPathFor: (operationId) => getPendingMutation(operationId)?.graphPath,
  });
  registerSyncApplicationEventPort({
    graphDelta: (graphPath) => { void invalidateGraphProjection(graphPath); },
    computationSettingsChanged: reconcileProjectComputationSettingsEvent,
    resourceMutationCommitted: async (result) => { await projectPublicationCoordinator.submit({ result: result as never }); },
    applyProjectLifecycleReceipt: async (result, dependencies) => {
      await applyProjectLifecycleReceipt(
        result as never,
        'event',
        dependencies as Parameters<typeof applyProjectLifecycleReceipt>[2]
          ?? createProjectLifecycleReceiptDependencies(),
      );
    },
    clearProject: () => {
      projectPublicationCoordinator.cancelProject();
      const owner = captureProjectLifecycleState();
      return createProjectLifecycleReceiptDependencies().clearProject(owner);
    },
  });
}
