import { useCallback } from 'react';
import { useProjectStore } from '@/features/project';
import { useNodeStore } from '@/features/node-registry/stores';
import { useLayoutStore } from '@/features/layoutStore/layoutStore';
import { ProjectService } from '@/services/project/projectService';
import { uiStore } from '@/features/ui/UIStore';

/**
 * Project Operations Hook
 * Handles save, load, and execute operations
 */
export function useProjectOperations(openSubGraph: (id: string, name: string, type: any, data?: any) => void) {
  const currentPath = useProjectStore((s) => s.currentPath);
  const setCurrentPath = useProjectStore((s) => s.setCurrentPath);

  const syncActiveToCollection = useCallback(() => {
    useProjectStore.getState().syncWithTabs(useNodeStore.getState().tabs);
  }, []);

  const saveGraphAs = useCallback(async () => {
    try {
      syncActiveToCollection();
      const st = useProjectStore.getState();
      const path = await ProjectService.saveProjectAs(
        st.globalVariables,
        st.events,
        st.functions,
        st.macros
      );
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
      const st = useProjectStore.getState();
      await ProjectService.saveProject(
        currentPath,
        st.globalVariables,
        st.events,
        st.functions,
        st.macros
      );
      uiStore.showToast("项目已保存", "success", 2000);
    } catch (e) {
      console.error(e);
      uiStore.showToast("保存失败", "error", 2000);
    }
  }, [currentPath, saveGraphAs, syncActiveToCollection]);

  const importGraph = useCallback(async (json?: string) => {
    try {
      let p: any;
      let path: string | null = null;

      if (json) {
        // 如果提供了 json，直接使用
        p = json;
        path = null;
        await ProjectService.setProjectData(p, path || undefined, true);
      } else {
        const result = await ProjectService.loadProject();
        if (!result) return;
        p = result.project;
        path = result.path;
      }

      useNodeStore.getState().clearTabs();

      const layoutStore = useLayoutStore.getState();
      const editorGroupId = layoutStore.activeEditorGroupId || 'default_editor';
      const editorNode = layoutStore.nodes[editorGroupId];
      if (editorNode?.data?.tabs) {
        layoutStore.updateNode(editorGroupId, {
          data: { ...editorNode.data, tabs: [], activeTabId: undefined }
        });
      }

      useProjectStore.getState().loadProject(p, path);

      const first = (Object.values(p.events)[0] || Object.values(p.functions)[0]) as any;
      if (first) openSubGraph(first.id, first.name, first.type as any, first);

      uiStore.showToast("项目已加载", "success", 2000);
    } catch (e) {
      console.error(e);
      uiStore.showToast("加载项目失败", "error", 3000);
    }
  }, [openSubGraph]);

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

      const st = useProjectStore.getState();
      const currentEvent = st.events[currentTabId];
      
      if (!currentEvent) {
        uiStore.showToast("只能执行 Event，当前打开的不是 Event", "warning", 3000);
        return;
      }

      const eventsToExecute = { [currentTabId]: currentEvent };
      console.log(`[Execute] 执行当前 Event: ${currentEvent.name} (${currentTabId})`);

      const res = await ProjectService.executeProject(
        st.globalVariables,
        eventsToExecute,
        st.functions,
        st.macros,
        st.dataframes
      );

      const logs = res.split('\n').filter(l => l.trim());
      logs.forEach(log => {
        if (log.includes("[Error]")) {
          uiStore.showToast(log, "error", 5000);
        } else if (log.includes("[NODE PRINT]")) {
          const printContent = log.replace(/.*\[NODE PRINT\]:\s*/, '');
          uiStore.showToast(`输出: ${printContent}`, "info", 3000);
          console.log(printContent);
        } else if (log.includes("[System] Received event")) {
          uiStore.showToast(log, "info", 2000);
        }
      });

      uiStore.showToast(`执行完成: ${currentEvent.name}`, "success", 2000);
    } catch (e) {
      console.error("执行失败:", e);
      uiStore.showToast(`执行失败: ${e}`, "error", 5000);
    }
  }, [syncActiveToCollection]);

  const executeAllEvents = useCallback(async () => {
    try {
      syncActiveToCollection();
      const st = useProjectStore.getState();

      const eventCount = Object.keys(st.events).length;
      if (eventCount === 0) {
        uiStore.showToast("没有可执行的 Event", "warning", 3000);
        return;
      }

      console.log(`[Execute] 执行所有 Events (共 ${eventCount} 个)`);

      const res = await ProjectService.executeProject(
        st.globalVariables,
        st.events,
        st.functions,
        st.macros,
        st.dataframes
      );

      const logs = res.split('\n').filter(l => l.trim());
      logs.forEach(log => {
        if (log.includes("[Error]")) {
          uiStore.showToast(log, "error", 5000);
        } else if (log.includes("[NODE PRINT]")) {
          const printContent = log.replace(/.*\[NODE PRINT\]:\s*/, '');
          uiStore.showToast(`输出: ${printContent}`, "info", 3000);
          console.log(printContent);
        } else if (log.includes("[System] Received event")) {
          uiStore.showToast(log, "info", 2000);
        }
      });

      uiStore.showToast(`执行完成: 共执行 ${eventCount} 个 Events`, "success", 2000);
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
