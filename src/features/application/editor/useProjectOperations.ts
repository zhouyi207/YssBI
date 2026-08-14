import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import {
  loadActivatedProject,
  resolveActiveProjectPath,
} from '@/features/core/dataStore';
import { editorDockviewPort } from '@/features/core/dockview';
import { getActiveLayoutTab, resolveEditorGroupId } from '@/features/core/layout/layoutTabQueries';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { markResourceDirty } from '@/features/core/resource';
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import { ProjectService, isExecutionCancelledError } from '@/services/project/projectService';
import { GraphService } from '@/services/graph/graphService';
import { saveAllDirtyGraphs } from './saveAllDirtyGraphs';
import { cancelActiveGraphRun } from './cancelActiveGraphRun';
import { observeGraphRunEvent, type GraphRunOutcomeState } from './observeGraphRunEvent';
import { openInspectableResult } from '@/features/application/execution/openInspectableResult';
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

function notifySaveAsSettlement(result: import('@/shared/types/dto/project').LifecycleMutationResultDto): void {
  if (result.outcome !== 'committed' || !result.record) {
    logger.notify.warn(`另存为需要恢复：${result.recovery?.action ?? result.outcome}`, "UI");
    return;
  }
  logger.notify.info(`项目已另存为：${result.record.name}`, "UI");
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
        logger.notify.warn("项目尚未加载", "UI");
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

      const result = await ProjectService.saveProjectAs(
        pending.projectInstanceId!,
        pending.operationId,
      );
      if (!pending.isCurrent()) return;
      if (!result) {
        cancelPendingProjectLifecycleOperation(pending.operationId);
        return;
      }
      const settlement = await applyProjectLifecycleReceipt(
        result,
        'direct',
        createProjectLifecycleReceiptDependencies(),
      );
      if (settlement.status === 'stale' || !pending.isCurrent()) return;
      const claimed = claimProjectLifecycleInitiatorSettlement(pending.operationId);
      if (claimed) notifySaveAsSettlement(claimed.result);
    } catch (e) {
      if (e instanceof ProjectLifecycleProtocolError && e.zeroEffects) return;
      if (pending) {
        const recovered = await recoverProjectLifecycleDirectFailure(pending.operationId);
        if (recovered && pending.isCurrent()) {
          const claimed = claimProjectLifecycleInitiatorSettlement(pending.operationId);
          if (claimed) notifySaveAsSettlement(claimed.result);
          return;
        }
        if (!pending.isCurrent()) return;
      }
      logger.app.error(String(e), 'ProjectOperations');
      logger.notify.error(`另存为失败：${formatErrorMessage(e)}`, "UI");
    }
  }, []);

  const saveGraph = useCallback(async () => {
    let context: GraphSaveCommandContext | undefined;
    const projectPath = await resolveActiveProjectPath();
    if (!projectPath) {
      logger.notify.warn("项目尚未加载", "UI");
      return;
    }
    try {
      const editorGroupId = resolveEditorGroupId();
      if (!editorGroupId) {
        logger.notify.warn("请先打开一个图或工作表", "UI");
        return;
      }

      const active = getActiveLayoutTab(editorGroupId);
      const activeTabId = active?.activeTabId;
      if (!activeTabId) {
        logger.notify.warn("请先打开一个图或工作表", "UI");
        return;
      }

      const activeTab = active?.tab;
      if (activeTab?.type === 'worksheet') {
        const saved = await useWorksheetStore.getState().saveDocument(activeTabId);
        if (saved) logger.notify.info(t('worksheet.saved'), "UI");
        return;
      }

      if (activeTab?.type !== 'event' && activeTab?.type !== 'function') {
        logger.notify.warn("请先打开一个图或工作表", "UI");
        return;
      }

      warnCallFunctionIssuesBeforeSave(activeTabId);

      context = await captureSettledGraphSaveCommandContext(activeTabId);
      await GraphService.saveProjectGraph(
        context.projectInstanceId,
        activeTabId,
        context.expectedRevision,
        context.operationId,
      );
      if (!isGraphSaveCommandRevisionCurrent(context, activeTabId)) return;
      markResourceDirty({ id: activeTabId, kind: activeTab.type }, false);
      logger.notify.info("图已保存", "UI");
    } catch (e) {
      if (context && !context.isCurrent()) return;
      logger.app.error(String(e), 'ProjectOperations');
      logger.notify.error(`保存失败：${formatErrorMessage(e)}`, "UI");
    }
  }, [t]);

  const importGraph = useCallback(async () => {
    try {
      const path = await ProjectService.pickProjectMetadataFile();
      if (!path) return;

      const activation = await ProjectService.loadProjectToState(path);

      const projectData = await loadActivatedProject(activation);
      if (!projectData) {
        logger.notify.error("加载项目失败", "UI");
        return;
      }

      // 清空 Dockview 中的当前编辑器面板，用户从侧栏自行打开资源。
      await editorDockviewPort.reset();

      logger.notify.info("项目已加载", "UI");
    } catch (e) {
      logger.app.error(String(e), 'ProjectOperations');
      logger.notify.error("加载项目失败", "UI");
    }
  }, []);

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
    if (!graphPath) {
      logger.notify.warn("请先打开一个 Event 才能执行", "UI");
      return;
    }

    const target = getExecutionEventGraph(graphPath);
    if (!target) {
      logger.notify.warn("只能执行 Event，当前打开的不是 Event", "UI");
      return;
    }

    const { graph: currentGraph } = target;
    let project: ProjectIdentitySnapshot;
    try {
      project = captureProjectIdentity();
    } catch {
      return;
    }

    try {
      logger.exec.info(`执行当前 Event: ${currentGraph.name} (${graphPath})`);

      const recording: RecordedEvent[] = [];
      const runState: GraphRunOutcomeState = { outcome: 'success' };
      useExecutionStore.getState().startExecution(graphPath);

      const result = await ProjectService.executeGraphDocument(
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
      );

      if (!isCurrentProjectIdentity(project)) return;
      finalizeExecutionRun(graphPath, recording, runState.outcome);
      logger.exec.debug(`执行 runId: ${result.runId}`);

      if (runState.outcome === 'cancelled') {
        logger.notify.warn(t('canvas.executionCancelled'), "UI");
      } else if (runState.outcome === 'error') {
        logger.notify.error(`执行失败: ${currentGraph.name}`, "UI");
      } else {
        logger.notify.info(`执行完成: ${currentGraph.name}`, "UI");
      }
    } catch (e) {
      if (!isCurrentProjectIdentity(project)) return;
      if (isExecutionCancelledError(e)) {
        logger.exec.info(`执行已中断: ${currentGraph.name} (${graphPath})`);
        finalizeExecutionRun(graphPath, [], 'cancelled');
        logger.notify.warn(t('canvas.executionCancelled'), "UI");
        return;
      }

      logger.exec.error(`执行失败: ${e instanceof Error ? e.message : String(e)}`);
      finalizeExecutionRun(graphPath, [], 'error');
      logger.notify.error(`执行失败: ${formatErrorMessage(e)}`, "UI");
    }
  }, [finalizeExecutionRun, t]);

  const cancelGraphExecution = useCallback(async (targetGraphPath?: string) => {
    const graphPath = resolveExecutionGraphPath(targetGraphPath);
    if (!graphPath) return;
    try {
      await cancelActiveGraphRun(graphPath);
    } catch (e) {
      logger.exec.error(`中断执行失败: ${formatErrorMessage(e)}`);
      logger.notify.error(`中断执行失败: ${formatErrorMessage(e)}`, "UI");
    }
  }, []);

  const clearGraphArtifacts = useCallback(async (targetGraphPath?: string) => {
    const graphPath = resolveExecutionGraphPath(targetGraphPath);
    if (!graphPath) {
      logger.notify.warn("请先打开一个 Event", "UI");
      return;
    }

    const store = useExecutionStore.getState();
    const graphState = store.getGraph(graphPath);
    if (graphState.status === "running") {
      return;
    }
    if (!graphHasClearableArtifacts(graphState)) {
      return;
    }

    store.clearGraphRunProjections(graphPath);
    logger.notify.info(t("canvas.executionArtifactsCleared"), "UI");
  }, [t]);

  return {
    saveGraph,
    saveGraphAs,
    importGraph,
    executeGraph,
    cancelGraphExecution,
    clearGraphArtifacts,
  };
}
