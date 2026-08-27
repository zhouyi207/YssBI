import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';

import {
  loadActivatedProject,
  resolveActiveProjectPath,
} from '@/features/core/dataStore';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { markResourceDirty } from '@/features/core/resource';
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import { ProjectService, isExecutionCancelledError } from '@/services/project/projectService';
import { openPathDialog } from '@/services/platform/pathDialog';
import { GraphService } from '@/services/graph/graphService';
import { saveAllDirtyGraphs } from './saveAllDirtyGraphs';
import { cancelActiveGraphRun } from './cancelActiveGraphRun';
import {
  observeGraphRunEvent,
  observeGraphRunOutput,
  type GraphRunOutcomeState,
} from './observeGraphRunEvent';
import { openInspectableResult } from '@/features/application/execution/openInspectableResult';
import {
  hasPendingGraphMutations,
  waitForPendingGraphMutations,
} from '@/features/application/editorMutation/pendingMutationRegistry';
import { resultRef } from '@/features/core/resultSource';
import { warnCallFunctionIssuesBeforeSave } from '@/features/application/graphDiagnostics/warnCallFunctionIssues';
import {
  captureSettledGraphSaveCommandContext,
  isGraphSaveCommandRevisionCurrent,
  type GraphSaveCommandContext,
} from '@/features/application/projectCommandContext';
import {
  useExecutionStore,
  getExecutionEventGraph,
  resolveExecutionGraphPath,
  graphHasClearableArtifacts,
} from '@/features/core/execution';

import type { RecordedEvent } from '@/shared/types/ui/execution';
import { ensureGraphExecutionTerminal } from '@/features/core/execution/executionRecording';
import { formatErrorMessage } from '@/shared/utils/formatErrorMessage';
import { logger } from '@/utils/appLogger';
import {
  ProjectLifecycleProtocolError,
  applyProjectLifecycleReceipt,
  cancelPendingProjectLifecycleOperation,
  claimProjectLifecycleInitiatorSettlement,
  recoverProjectLifecycleDirectFailure,
  registerPendingProjectLifecycleOperation,
  type PendingProjectLifecycleOperation,
} from '@/features/application/projectLifecycleReceipt';
import { createProjectLifecycleReceiptDependencies } from '@/features/application/projectLifecycleReceiptDependencies';
import { showBlockingIpcError, showBlockingMessage } from './blockingErrorDialog';
import {
  captureActiveEditorCommandTarget,
  isEditorCommandTargetCurrent,
  type EditorCommandTarget,
} from './editorCommandFocus';

function projectParentDirectory(metadataOrRootPath: string): string {
  const normalized = metadataOrRootPath.replace(/\\/g, '/');
  const root = normalized.replace(/\/metadata\.yssbi$/i, '');
  const index = root.lastIndexOf('/');
  return index > 0 ? root.slice(0, index) : root;
}

/**
 * Project Operations Hook
 * Handles flush, load, and execute operations
 */
export function useProjectOperations() {
  const { t } = useTranslation();

  const saveGraphAs = useCallback(async () => {
    let pending: PendingProjectLifecycleOperation | undefined;
    try {
      pending = registerPendingProjectLifecycleOperation({ kind: 'saveAs' });
      const projectPath = await resolveActiveProjectPath();
      if (!pending.isCurrent()) {
        cancelPendingProjectLifecycleOperation(pending.operationId);
        return;
      }
      if (!projectPath) {
        cancelPendingProjectLifecycleOperation(pending.operationId);
        showBlockingMessage(t('notifications.project.notLoaded'));
        return;
      }
      const dirtySaved = await saveAllDirtyGraphs();
      if (!pending.isCurrent()) {
        cancelPendingProjectLifecycleOperation(pending.operationId);
        return;
      }
      if (!dirtySaved) {
        cancelPendingProjectLifecycleOperation(pending.operationId);
        return;
      }

      const currentPath = await ProjectService.getProjectPath(pending.projectInstanceId!);
      if (!pending.isCurrent()) return;
      if (!currentPath) {
        cancelPendingProjectLifecycleOperation(pending.operationId);
        showBlockingMessage(t('notifications.project.notLoaded'));
        return;
      }

      const selection = await openPathDialog({
        directory: true,
        multiple: false,
        title: '项目另存为',
        defaultPath: projectParentDirectory(currentPath) || undefined,
      });
      if (!pending.isCurrent()) return;
      if (!selection.ok) {
        cancelPendingProjectLifecycleOperation(pending.operationId);
        showBlockingMessage(t('notifications.project.saveAsFailed', {
          error: selection.failure.code,
        }));
        return;
      }
      const destination = selection.value;
      if (!destination || Array.isArray(destination)) {
        cancelPendingProjectLifecycleOperation(pending.operationId);
        return;
      }

      const result = await ProjectService.saveProjectAs(
        pending.projectInstanceId!,
        pending.operationId,
        destination,
      );
      if (!pending.isCurrent()) return;
      const settlement = await applyProjectLifecycleReceipt(
        result,
        'direct',
        createProjectLifecycleReceiptDependencies(),
      );
      if (settlement.status === 'stale' || !pending.isCurrent()) return;
      claimProjectLifecycleInitiatorSettlement(pending.operationId);
    } catch (e) {
      if (e instanceof ProjectLifecycleProtocolError && e.zeroEffects) return;
      if (pending) {
        const recovered = await recoverProjectLifecycleDirectFailure(pending.operationId);
        if (recovered && pending.isCurrent()) {
          claimProjectLifecycleInitiatorSettlement(pending.operationId);
          return;
        }
        if (!pending.isCurrent()) return;
      }
      logger.app.error(String(e), 'ProjectOperations');
      showBlockingIpcError(e, 'save_project_as', (code) =>
        t('notifications.project.saveAsFailed', { error: code }));
    }
  }, [t]);

  const saveGraph = useCallback(async (requestedTarget?: EditorCommandTarget) => {
    const target = requestedTarget ?? captureActiveEditorCommandTarget();
    if (!target) {
      showBlockingMessage(t('notifications.project.openResourceBeforeSaving'));
      return;
    }
    if (!isEditorCommandTargetCurrent(target)) return;

    let context: GraphSaveCommandContext | undefined;
    try {
      const projectPath = await resolveActiveProjectPath();
      if (!isEditorCommandTargetCurrent(target)) return;
      if (!projectPath) {
        showBlockingMessage(t('notifications.project.notLoaded'));
        return;
      }

      if (target.resourceKind === 'worksheet') {
        const saved = await useWorksheetStore.getState().saveDocument(target.resourceRef);
        if (!isEditorCommandTargetCurrent(target)) return;
        if (!saved) {
          showBlockingMessage(t('notifications.project.saveFailed', {
            error: 'worksheet_save_not_committed',
          }));
        }
        return;
      }

      warnCallFunctionIssuesBeforeSave(target.resourceRef);

      context = await captureSettledGraphSaveCommandContext(target.resourceRef);
      if (!isEditorCommandTargetCurrent(target)) return;
      await GraphService.saveProjectGraph(
        context.projectInstanceId,
        target.resourceRef,
        context.expectedRevision,
        context.operationId,
      );
      if (!isEditorCommandTargetCurrent(target)) return;
      if (!isGraphSaveCommandRevisionCurrent(context, target.resourceRef)) return;
      markResourceDirty({ id: target.resourceRef, kind: target.resourceKind }, false);
    } catch (e) {
      if (!isEditorCommandTargetCurrent(target) || (context && !context.isCurrent())) return;
      logger.app.error(String(e), 'ProjectOperations');
      showBlockingIpcError(e, 'save_project_graph', (code) =>
        t('notifications.project.saveFailed', { error: code }));
    }
  }, [t]);

  const importGraph = useCallback(async () => {
    try {
      const selection = await openPathDialog({
        multiple: false,
        filters: [{ name: 'YssBI Project', extensions: ['yssbi'] }],
      });
      if (!selection.ok) {
        showBlockingMessage(`${t('notifications.project.loadFailed')} (${selection.failure.code})`);
        return;
      }
      const path = selection.value;
      if (!path) return;
      if (Array.isArray(path)) return;

      const activation = await ProjectService.loadProjectToState(path);

      const projectData = await loadActivatedProject(activation);
      if (!projectData) {
        showBlockingMessage(t('notifications.project.loadFailed'));
        return;
      }

    } catch (e) {
      logger.app.error(String(e), 'ProjectOperations');
      showBlockingIpcError(e, 'load_project_to_state', (code) =>
        `${t('notifications.project.loadFailed')} (${code})`);
    }
  }, [t]);

  const finalizeExecutionRun = useCallback((
    graphPath: string,
    recording: RecordedEvent[],
    outcome: 'success' | 'cancelled' | 'error',
  ) => {
    const store = useExecutionStore.getState();

    if (outcome === 'cancelled') {
      store.interruptExecution(graphPath);
      return;
    }

    store.commitExecutionVisual(graphPath);

    if (recording.length > 0) {
      store.setRecording(graphPath, recording);
    }

    ensureGraphExecutionTerminal(graphPath, outcome === 'error' ? 'error' : 'success');
  }, []);

  const executeGraph = useCallback(async (targetGraphPath?: string) => {
    const graphPath = resolveExecutionGraphPath(targetGraphPath);
    if (!graphPath) return;

    const target = getExecutionEventGraph(graphPath);
    if (!target) return;

    const { graph: currentGraph } = target;
    let project: ProjectIdentitySnapshot;
    try {
      project = captureProjectIdentity();
    } catch {
      return;
    }

    if (hasPendingGraphMutations(graphPath)) {
      await waitForPendingGraphMutations(graphPath);
      if (!isCurrentProjectIdentity(project)) return;
    }

    try {
      logger.exec.info(`执行当前 Event: ${currentGraph.name} (${graphPath})`);

      const recording: RecordedEvent[] = [];
      const runState: GraphRunOutcomeState = { outcome: 'success' };
      useExecutionStore.getState().startExecution(graphPath);

      await ProjectService.executeGraphDocument(
        project.projectInstanceId,
        graphPath,
        { type: 'default' },
        (event) => {
          if (!isCurrentProjectIdentity(project)) return;
          observeGraphRunEvent(graphPath, event, runState);
          if (event.kind.type === 'openResultWindow') {
            void openInspectableResult(resultRef(event.kind.resultId), t);
          }
        },
        (event) => {
          if (!isCurrentProjectIdentity(project)) return;
          observeGraphRunOutput(graphPath, event);
        },
      );

      if (!isCurrentProjectIdentity(project)) return;
      finalizeExecutionRun(graphPath, recording, runState.outcome);
    } catch (e) {
      if (!isCurrentProjectIdentity(project)) return;
      if (isExecutionCancelledError(e)) {
        logger.exec.info(`执行已中断: ${currentGraph.name} (${graphPath})`);
        finalizeExecutionRun(graphPath, [], 'cancelled');
        return;
      }

      logger.exec.error(`执行失败: ${e instanceof Error ? e.message : String(e)}`);
      finalizeExecutionRun(graphPath, [], 'error');
    }
  }, [finalizeExecutionRun, t]);

  const cancelGraphExecution = useCallback(async (targetGraphPath?: string) => {
    const graphPath = resolveExecutionGraphPath(targetGraphPath);
    if (!graphPath) return;
    try {
      await cancelActiveGraphRun(graphPath);
    } catch (e) {
      logger.exec.error(`中断执行失败: ${formatErrorMessage(e)}`);
    }
  }, []);

  const clearGraphArtifacts = useCallback(async (targetGraphPath?: string) => {
    const graphPath = resolveExecutionGraphPath(targetGraphPath);
    if (!graphPath) return;

    const store = useExecutionStore.getState();
    const graphState = store.getGraph(graphPath);
    if (graphState.status === "running") {
      return;
    }
    if (!graphHasClearableArtifacts(graphState)) {
      return;
    }

    store.clearGraphRunProjections(graphPath);
  }, []);

  return {
    saveGraph,
    saveGraphAs,
    importGraph,
    executeGraph,
    cancelGraphExecution,
    clearGraphArtifacts,
  };
}
