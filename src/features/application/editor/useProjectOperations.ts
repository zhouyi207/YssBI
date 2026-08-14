import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import {
  loadActivatedProject,
  resolveActiveProjectPath,
  useGraphDataStore,
} from '@/features/core/dataStore';
import { listEditorGroupTabIds } from '@/features/core/layout/editorTabStore';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { getActiveLayoutTab, resolveEditorGroupId, resolveEditorTargetGroupId } from '@/features/core/layout/layoutTabQueries';
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
import { openInspectableSource } from '@/features/application/execution/openInspectableSource';
import { windowSourceRef } from '@/features/core/resultSource';
import { warnCallFunctionIssuesBeforeSave } from '@/features/application/graphDiagnostics/warnCallFunctionIssues';
import {
  captureSettledGraphSaveCommandContext,
  isGraphSaveCommandRevisionCurrent,
  type GraphSaveCommandContext,
} from '@/features/application/projectCommandContext';
import { uiStore } from '@/features/core/ui/UIStore';
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
    uiStore.showToast(`另存为需要恢复：${result.recovery?.action ?? result.outcome}`, "warning", 4000);
    return;
  }
  uiStore.showToast(`项目已另存为：${result.record.name}`, "success", 3000);
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
        uiStore.showToast("项目尚未加载", "warning", 2000);
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
      uiStore.showToast(`另存为失败：${formatErrorMessage(e)}`, "error", 3000);
    }
  }, []);

  const saveGraph = useCallback(async () => {
    let context: GraphSaveCommandContext | undefined;
    const projectPath = await resolveActiveProjectPath();
    if (!projectPath) {
      uiStore.showToast("项目尚未加载", "warning", 2000);
      return;
    }
    try {
      const layoutStore = useLayoutStore.getState();
      const editorGroupId = resolveEditorGroupId(undefined, layoutStore);
      if (!editorGroupId) {
        uiStore.showToast("请先打开一个图或工作表", "warning", 2000);
        return;
      }

      const active = getActiveLayoutTab(editorGroupId, layoutStore.nodes);
      const activeTabId = active?.activeTabId;
      if (!activeTabId) {
        uiStore.showToast("请先打开一个图或工作表", "warning", 2000);
        return;
      }

      const activeTab = active?.tab;
      if (activeTab?.type === 'worksheet') {
        const saved = await useWorksheetStore.getState().saveDocument(activeTabId);
        if (saved) uiStore.showToast(t('worksheet.saved'), 'success', 2000);
        return;
      }

      if (activeTab?.type !== 'event' && activeTab?.type !== 'function') {
        uiStore.showToast("请先打开一个图或工作表", "warning", 2000);
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
      uiStore.showToast("图已保存", "success", 2000);
    } catch (e) {
      if (context && !context.isCurrent()) return;
      logger.app.error(String(e), 'ProjectOperations');
      uiStore.showToast(`保存失败：${formatErrorMessage(e)}`, "error", 2000);
    }
  }, [t]);

  const importGraph = useCallback(async () => {
    try {
      const path = await ProjectService.pickProjectMetadataFile();
      if (!path) return;

      const activation = await ProjectService.loadProjectToState(path);

      const projectData = await loadActivatedProject(activation);
      if (!projectData) {
        uiStore.showToast("加载项目失败", "error", 3000);
        return;
      }

      // 清空当前 tabs，用户从侧栏自行打开资源
      const layoutStore = useLayoutStore.getState();
      const editorGroupId = resolveEditorTargetGroupId(undefined, layoutStore.nodes, layoutStore);
      for (const tabId of [...listEditorGroupTabIds(editorGroupId)]) {
        layoutStore.removeTab(editorGroupId, tabId);
      }

      uiStore.showToast("项目已加载", "success", 2000);
    } catch (e) {
      logger.app.error(String(e), 'ProjectOperations');
      uiStore.showToast("加载项目失败", "error", 3000);
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
      uiStore.showToast("请先打开一个 Event 才能执行", "warning", 3000);
      return;
    }

    const target = getExecutionEventGraph(graphPath);
    if (!target) {
      uiStore.showToast("只能执行 Event，当前打开的不是 Event", "warning", 3000);
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
          if (event.kind.type !== 'outputReady' || event.kind.generation !== null) return;
          const node = useGraphDataStore.getState().graphEntities[graphPath]
            ?.nodes[event.kind.output.port.nodeId];
          if (node?.nodeType === 'yssbi.debug.view') {
            void openInspectableSource(windowSourceRef(event.kind.sourceId), t);
          }
        },
      );

      if (!isCurrentProjectIdentity(project)) return;
      finalizeExecutionRun(graphPath, recording, runState.outcome);
      logger.exec.debug(`执行 runId: ${result.runId}`);

      if (runState.outcome === 'cancelled') {
        uiStore.showToast(t('canvas.executionCancelled'), "warning", 2500);
      } else if (runState.outcome === 'error') {
        uiStore.showToast(`执行失败: ${currentGraph.name}`, "error", 5000);
      } else {
        uiStore.showToast(`执行完成: ${currentGraph.name}`, "success", 2000);
      }
    } catch (e) {
      if (!isCurrentProjectIdentity(project)) return;
      if (isExecutionCancelledError(e)) {
        logger.exec.info(`执行已中断: ${currentGraph.name} (${graphPath})`);
        finalizeExecutionRun(graphPath, [], 'cancelled');
        uiStore.showToast(t('canvas.executionCancelled'), "warning", 2500);
        return;
      }

      logger.exec.error(`执行失败: ${e instanceof Error ? e.message : String(e)}`);
      finalizeExecutionRun(graphPath, [], 'error');
      uiStore.showToast(`执行失败: ${formatErrorMessage(e)}`, "error", 5000);
    }
  }, [finalizeExecutionRun, t]);

  const cancelGraphExecution = useCallback(async (targetGraphPath?: string) => {
    const graphPath = resolveExecutionGraphPath(targetGraphPath);
    if (!graphPath) return;
    try {
      await cancelActiveGraphRun(graphPath);
    } catch (e) {
      logger.exec.error(`中断执行失败: ${formatErrorMessage(e)}`);
      uiStore.showToast(`中断执行失败: ${formatErrorMessage(e)}`, "error", 3000);
    }
  }, []);

  const clearGraphArtifacts = useCallback(async (targetGraphPath?: string) => {
    const graphPath = resolveExecutionGraphPath(targetGraphPath);
    if (!graphPath) {
      uiStore.showToast("请先打开一个 Event", "warning", 3000);
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

    try {
      await ProjectService.clearGraphExecutionArtifacts(graphPath);
      store.clearGraphRunArtifacts(graphPath);
      uiStore.showToast(t("canvas.executionArtifactsCleared"), "success", 2000);
    } catch (e) {
      logger.exec.error(`清除运行结果失败: ${formatErrorMessage(e)}`);
      uiStore.showToast(
        t("canvas.executionArtifactsClearFailed", { message: formatErrorMessage(e) }),
        "error",
        3000,
      );
    }
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
