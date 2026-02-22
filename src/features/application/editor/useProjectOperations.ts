import { useCallback } from 'react';
import { useProjectIOStore, getGraphById } from '@/features/core/dataStore';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { ProjectService } from '@/services/project/projectService';
import { uiStore } from '@/features/core/ui/UIStore';
import { useExecutionStore } from '@/features/core/execution';
import type { ExecutionEvent, RecordedEvent } from '@/shared/types/ui/execution';

/**
 * Project Operations Hook
 * Handles save, load, and execute operations
 */
export function useProjectOperations(openGraph: (id: string, name: string, type: any, data?: any) => void) {
  const currentPath = useProjectIOStore((s) => s.currentPath);
  const setCurrentPath = useProjectIOStore((s) => s.setCurrentPath);

  // 注意：新架构中不需要 syncActiveToCollection，后端事件会自动同步
  const syncActiveToCollection = useCallback(() => {
    // TODO: 如果需要，实现新的同步逻辑
    console.log('[ProjectOperations] syncActiveToCollection called (no-op in new architecture)');
  }, []);

  const saveGraphAs = useCallback(async () => {
    try {
      syncActiveToCollection();
      const path = await ProjectService.saveProjectFromState();
      if (path) {
        setCurrentPath(path);
        uiStore.showToast("项目已保存", "success", 2000);
      }
    } catch (e) {
      console.error(e);
    }
  }, [syncActiveToCollection, setCurrentPath]);

  const saveGraph = useCallback(async () => {
    if (!currentPath) return saveGraphAs();
    syncActiveToCollection();
    try {
      await ProjectService.saveProjectFromState(currentPath);
      uiStore.showToast("项目已保存", "success", 2000);
    } catch (e) {
      console.error(e);
      uiStore.showToast("保存失败", "error", 2000);
    }
  }, [currentPath, saveGraphAs, syncActiveToCollection]);

  const importGraph = useCallback(async () => {
    try {
      const result = await ProjectService.loadProjectToState();
      if (!result) return; // 用户取消选择文件

      const projectData = await useProjectIOStore.getState().syncFromBackend();
      if (!projectData) {
        uiStore.showToast("加载项目失败", "error", 3000);
        return;
      }

      // 清空当前 tabs，再打开新项目的第一个 graph
      const layoutStore = useLayoutStore.getState();
      const editorGroupId = layoutStore.activeEditorGroupId || 'default_editor';
      const editorNode = layoutStore.nodes[editorGroupId];
      if (editorNode?.data?.tabs) {
        layoutStore.updateNode(editorGroupId, {
          data: { ...editorNode.data, tabs: [], activeTabId: undefined }
        });
      }

      const first = Object.values(projectData.graphs)[0] as any;
      if (first) openGraph(first.id, first.name, first.type as any, first);

      uiStore.showToast("项目已加载", "success", 2000);
    } catch (e) {
      console.error(e);
      uiStore.showToast("加载项目失败", "error", 3000);
    }
  }, [openGraph]);

  const executeGraph = useCallback(async () => {
    try {
      syncActiveToCollection();

      const layoutStore = useLayoutStore.getState();
      const editorGroupId = layoutStore.activeEditorGroupId || layoutStore.activeGroupId;
      const editorNode = editorGroupId ? layoutStore.nodes[editorGroupId] : null;
      const currentTabId = editorNode?.data?.activeTabId;

      if (!currentTabId) {
        uiStore.showToast("请先打开一个 Event 才能执行", "warning", 3000);
        return;
      }

      const currentGraph = getGraphById(currentTabId);
      
      if (!currentGraph || currentGraph.type !== 'event') {
        uiStore.showToast("只能执行 Event，当前打开的不是 Event", "warning", 3000);
        return;
      }

      console.log(`[Execute] 执行当前 Event: ${currentGraph.name} (${currentTabId})`);

      const recording: RecordedEvent[] = [];
      const { applyEvent, setRecording } = useExecutionStore.getState();

      const res = await ProjectService.executeProject((event: ExecutionEvent) => {
        applyEvent(event);
        recording.push({ event, timestamp: Date.now() });
      });

      setRecording(recording);

      if (res.logs.length > 0) {
        console.log("[Execute] 执行日志:");
        res.logs.forEach((line: string) => console.log("  ", line));
      }
      
      uiStore.showToast(`执行完成: ${currentGraph.name}`, "success", 2000);
    } catch (e) {
      console.error("执行失败:", e);
      uiStore.showToast(`执行失败: ${e}`, "error", 5000);
    }
  }, [syncActiveToCollection]);

  const executeAllEvents = useCallback(async () => {
    try {
      syncActiveToCollection();
      const snapshot = useProjectIOStore.getState().exportSnapshot();
      const events = Object.values(snapshot.graphs).filter((g: any) => g?.type === 'event');
      const eventCount = events.length;
      
      if (eventCount === 0) {
        uiStore.showToast("没有可执行的 Event", "warning", 3000);
        return;
      }

      console.log(`[Execute] 执行所有 Events (共 ${eventCount} 个)`);

      const recording: RecordedEvent[] = [];
      const { applyEvent, setRecording } = useExecutionStore.getState();

      const res = await ProjectService.executeProject((event: ExecutionEvent) => {
        applyEvent(event);
        recording.push({ event, timestamp: Date.now() });
      });

      setRecording(recording);

      if (res.logs.length > 0) {
        console.log("[Execute] 执行日志:");
        res.logs.forEach((line: string) => console.log("  ", line));
      }

      uiStore.showToast(`执行完成: 共执行 ${res.executedGraphs} 个 Events`, "success", 2000);
    } catch (e) {
      console.error("执行失败:", e);
      uiStore.showToast(`执行失败: ${e}`, "error", 5000);
    }
  }, [syncActiveToCollection]);

  return {
    saveGraph,
    saveGraphAs,
    importGraph,
    executeGraph,
    executeAllEvents,
  };
}
