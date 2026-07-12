import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useProjectIOStore } from '@/features/core/dataStore';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { getActiveLayoutTab, resolveEditorGroupId, resolveEditorTargetGroupId } from '@/features/core/layout/layoutTabQueries';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { markResourceDirty } from '@/features/core/resource';
import { ProjectService, isExecutionCancelledError } from '@/services/project/projectService';
import { GraphService } from '@/services/graph/graphService';
import { saveAllDirtyGraphs } from './saveAllDirtyGraphs';
import { warnCallFunctionIssuesBeforeSave } from '@/features/application/graphDiagnostics/warnCallFunctionIssues';
import { uiStore } from '@/features/core/ui/UIStore';
import {
  useExecutionStore,
  getExecutionEventGraph,
  resolveExecutionGraphPath,
  graphHasClearableArtifacts,
  enqueueLiveExecutionEvent,
} from '@/features/core/execution';
import { openWindowInspectableSource } from '@/features/application/execution/openInspectableSource';
import type { Presentation } from '@/features/core/resultSource';
import type { ExecutionEvent, RecordedEvent } from '@/shared/types/ui/execution';
import {
  ensureGraphExecutionTerminal,
  firstNodeErrorMessage,
  recordingHadError,
} from '@/features/core/execution/executionRecording';
import { formatErrorMessage } from '@/shared/utils/formatErrorMessage';
import { logger } from '@/utils/appLogger';

/**
 * Project Operations Hook
 * Handles flush, load, and execute operations
 */
export function useProjectOperations() {
  const { t } = useTranslation();
  const currentPath = useProjectIOStore((s) => s.currentPath);

  const saveGraphAs = useCallback(async () => {
    if (!currentPath) {
      uiStore.showToast("项目尚未加载", "warning", 2000);
      return;
    }
    try {
      const dirtySaved = await saveAllDirtyGraphs();
      if (!dirtySaved) return;

      const record = await ProjectService.saveProjectAs();
      if (!record) return;

      await useProjectIOStore.getState().loadProject();
      uiStore.showToast(`项目已另存为：${record.name}`, "success", 3000);
    } catch (e) {
      logger.app.error(String(e), 'ProjectOperations');
      uiStore.showToast(`另存为失败：${formatErrorMessage(e)}`, "error", 3000);
    }
  }, [currentPath]);

  const saveGraph = useCallback(async () => {
    if (!currentPath) {
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
        await useWorksheetStore.getState().saveDocument(activeTabId);
        uiStore.showToast(t('worksheet.saved'), 'success', 2000);
        return;
      }

      if (activeTab?.type !== 'event' && activeTab?.type !== 'function') {
        uiStore.showToast("请先打开一个图或工作表", "warning", 2000);
        return;
      }

      warnCallFunctionIssuesBeforeSave(activeTabId);

      const savedPath = await GraphService.saveProjectGraph(activeTabId);
      markResourceDirty({ id: savedPath, kind: activeTab.type }, false);
      uiStore.showToast("图已保存", "success", 2000);
    } catch (e) {
      logger.app.error(String(e), 'ProjectOperations');
      uiStore.showToast(`保存失败：${formatErrorMessage(e)}`, "error", 2000);
    }
  }, [currentPath, t]);

  const importGraph = useCallback(async () => {
    try {
      const path = await ProjectService.pickProjectMetadataFile();
      if (!path) return;

      await ProjectService.loadProjectToState(path);

      const projectData = await useProjectIOStore.getState().loadProject();
      if (!projectData) {
        uiStore.showToast("加载项目失败", "error", 3000);
        return;
      }

      // 清空当前 tabs，用户从侧栏自行打开资源
      const layoutStore = useLayoutStore.getState();
      const editorGroupId = resolveEditorTargetGroupId(undefined, layoutStore.nodes, layoutStore);
      const editorNode = layoutStore.nodes[editorGroupId];
      if (editorNode?.data?.tabs) {
        layoutStore.updateNode(editorGroupId, {
          data: { ...editorNode.data, tabs: [], activeTabId: undefined }
        });
      }

      uiStore.showToast("项目已加载", "success", 2000);
    } catch (e) {
      logger.app.error(String(e), 'ProjectOperations');
      uiStore.showToast("加载项目失败", "error", 3000);
    }
  }, []);

  const handleOpenSourceWindow = useCallback(async (
    sourceId: string,
    event: { presentation: Presentation; windowTitle: string },
  ) => {
    await openWindowInspectableSource(sourceId, event);
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

    try {
      logger.exec.info(`执行当前 Event: ${currentGraph.name} (${graphPath})`);

      const recording: RecordedEvent[] = [];
      const pendingWindows: Promise<void>[] = [];
      const { startExecution, applySideEffectEvent } = useExecutionStore.getState();
      startExecution(graphPath);

      const res = await ProjectService.executeProject((event: ExecutionEvent) => {
        if (event.event === 'openSourceWindow') {
          pendingWindows.push(
            handleOpenSourceWindow(event.data.sourceId, {
              presentation: event.data.presentation,
              windowTitle: event.data.windowTitle,
            }),
          );
          return;
        }
        enqueueLiveExecutionEvent(graphPath, event, (_gid, e) => applySideEffectEvent(graphPath, e));
        recording.push({ event, timestamp: Date.now() });
      }, graphPath);

      await Promise.all(pendingWindows);
      const hadError = recordingHadError(recording);
      finalizeExecutionRun(graphPath, recording, hadError ? 'error' : 'success');

      if (res.logs.length > 0) {
        logger.exec.debug(`执行日志:\n${res.logs.map((line: string) => `  ${line}`).join('\n')}`);
      }

      if (hadError) {
        const nodeErr = firstNodeErrorMessage(recording);
        uiStore.showToast(
          nodeErr ? `执行失败: ${nodeErr}` : `执行失败: ${currentGraph.name}`,
          "error",
          5000,
        );
      } else {
        uiStore.showToast(`执行完成: ${currentGraph.name}`, "success", 2000);
      }
    } catch (e) {
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
  }, [handleOpenSourceWindow, finalizeExecutionRun, t]);

  const cancelGraphExecution = useCallback(async () => {
    try {
      await ProjectService.cancelExecution();
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
