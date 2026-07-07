import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useProjectIOStore } from '@/features/core/dataStore';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { getActiveLayoutTab, resolveEditorGroupId } from '@/features/core/layout/layoutTabQueries';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { markResourceDirty } from '@/features/core/resource';
import { ProjectService, isExecutionCancelledError } from '@/services/project/projectService';
import { GraphService } from '@/services/graph/graphService';
import { saveAllDirtyGraphs } from './saveAllDirtyGraphs';
import { uiStore } from '@/features/core/ui/UIStore';
import { useExecutionStore, getExecutionEventGraph, resolveExecutionGraphId } from '@/features/core/execution';
import { openPresentationWindowSafe } from '@/features/application/window';
import { plotTypeFromPresentation, presentationRoute } from '@/features/core/resultSource';
import type { Presentation } from '@/features/core/resultSource';
import type { ExecutionEvent, RecordedEvent } from '@/shared/types/ui/execution';
import { enqueueLiveExecutionEvent, flushLiveExecutionEventsNow, resetExecutionVisual } from '@/features/core/execution';
import { formatErrorMessage } from '@/shared/utils/formatErrorMessage';
import { logger } from '@/utils/appLogger';

/**
 * Project Operations Hook
 * Handles flush, load, and execute operations
 */
export function useProjectOperations() {
  const { t } = useTranslation();
  const currentPath = useProjectIOStore((s) => s.currentPath);

  // 注意：新架构中不需要 syncActiveToCollection，后端事件会自动同步
  const syncActiveToCollection = useCallback(() => {
    // TODO: 如果需要，实现新的同步逻辑
    logger.app.debug('syncActiveToCollection called (no-op in new architecture)', 'ProjectOperations');
  }, []);

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
    syncActiveToCollection();
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

      await GraphService.saveProjectGraph(activeTabId);
      markResourceDirty({ id: activeTabId, kind: activeTab.type }, false);
      uiStore.showToast("图已保存", "success", 2000);
    } catch (e) {
      logger.app.error(String(e), 'ProjectOperations');
      uiStore.showToast(`保存失败：${formatErrorMessage(e)}`, "error", 2000);
    }
  }, [currentPath, syncActiveToCollection, t]);

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
      const editorGroupId = layoutStore.activeEditorGroupId || 'default_editor';
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
    await openPresentationWindowSafe(
      sourceId,
      {
        route: presentationRoute(event.presentation),
        windowTitle: event.windowTitle,
        plotType: plotTypeFromPresentation(event.presentation),
      },
      'ProjectOperations',
    );
  }, []);

  const finalizeExecutionRun = useCallback((
    graphId: string,
    recording: RecordedEvent[],
    outcome: 'success' | 'cancelled' | 'error',
  ) => {
    flushLiveExecutionEventsNow();
    const store = useExecutionStore.getState();
    store.commitExecutionVisual(graphId);

    if (outcome === 'success') {
      store.setRecording(graphId, recording);
      if (store.graphs[graphId]?.status === 'running') {
        store.completeExecution(graphId);
      }
      return;
    }

    if (outcome === 'cancelled') {
      store.interruptExecution(graphId);
      return;
    }

    store.failExecution(graphId);
  }, []);

  const executeGraph = useCallback(async (targetGraphId?: string) => {
    const graphId = resolveExecutionGraphId(targetGraphId);
    if (!graphId) {
      uiStore.showToast("请先打开一个 Event 才能执行", "warning", 3000);
      return;
    }

    const target = getExecutionEventGraph(graphId);
    if (!target) {
      uiStore.showToast("只能执行 Event，当前打开的不是 Event", "warning", 3000);
      return;
    }

    const { graph: currentGraph } = target;

    try {
      syncActiveToCollection();
      logger.exec.info(`执行当前 Event: ${currentGraph.name} (${graphId})`);

      const recording: RecordedEvent[] = [];
      const pendingWindows: Promise<void>[] = [];
      const { startExecution, applySideEffectEvent } = useExecutionStore.getState();
      resetExecutionVisual(graphId);
      startExecution(graphId);

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
        enqueueLiveExecutionEvent(graphId, event, (_gid, e) => applySideEffectEvent(graphId, e));
        recording.push({ event, timestamp: Date.now() });
      }, graphId);

      await Promise.all(pendingWindows);
      finalizeExecutionRun(graphId, recording, 'success');

      if (res.logs.length > 0) {
        logger.exec.debug(`执行日志:\n${res.logs.map((line: string) => `  ${line}`).join('\n')}`);
      }

      uiStore.showToast(`执行完成: ${currentGraph.name}`, "success", 2000);
    } catch (e) {
      if (isExecutionCancelledError(e)) {
        logger.exec.info(`执行已中断: ${currentGraph.name} (${graphId})`);
        finalizeExecutionRun(graphId, [], 'cancelled');
        uiStore.showToast(t('canvas.executionCancelled'), "warning", 2500);
        return;
      }

      logger.exec.error(`执行失败: ${e instanceof Error ? e.message : String(e)}`);
      finalizeExecutionRun(graphId, [], 'error');
      uiStore.showToast(`执行失败: ${formatErrorMessage(e)}`, "error", 5000);
    }
  }, [handleOpenSourceWindow, syncActiveToCollection, finalizeExecutionRun, t]);

  const cancelGraphExecution = useCallback(async () => {
    try {
      await ProjectService.cancelExecution();
    } catch (e) {
      logger.exec.error(`中断执行失败: ${formatErrorMessage(e)}`);
      uiStore.showToast(`中断执行失败: ${formatErrorMessage(e)}`, "error", 3000);
    }
  }, []);

  return {
    saveGraph,
    saveGraphAs,
    importGraph,
    executeGraph,
    cancelGraphExecution,
  };
}
