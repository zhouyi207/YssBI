import { useEffect, useCallback } from "react";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { triggerImportData } from "@/features/application/dataManagement/useDatabaseManagement";
import { SettingsService } from "@/services/settings/settingsService";
import { DEFAULT_WINDOW } from "@/app/appConfig/default";

/**
 * Menubar logic: window lifecycle, import, split, open windows, etc.
 * Extracted from Menubar.tsx - view should only consume this hook.
 */
export function useMenubar() {
  const openSettings = useLayoutStore((s) => s.openSettings);
  const activeEditorGroupId = useLayoutStore((s) => s.activeEditorGroupId);
  const splitNode = useLayoutStore((s) => s.splitNode);
  const detailNode = useLayoutStore((s) => s.nodes["detail"]);
  const updateNode = useLayoutStore((s) => s.updateNode);

  const isDetailVisible = detailNode?.data?.visible !== false;

  useEffect(() => {
    const appWindow = getCurrentWindow();
    let unlistenClose: (() => void) | null = null;

    const setupCloseListener = async () => {
      unlistenClose = await appWindow.onCloseRequested(async () => {
        try {
          const isMaximized = await appWindow.isMaximized();
          if (isMaximized) {
            const settings = await SettingsService.loadSettings();
            await SettingsService.saveSettings({
              ...settings,
              window: {
                ...settings.window,
                isMaximized: true,
                width: DEFAULT_WINDOW.width,
                height: DEFAULT_WINDOW.height,
              },
            });
          } else {
            const size = await appWindow.innerSize();
            const position = await appWindow.outerPosition();
            const settings = await SettingsService.loadSettings();
            await SettingsService.saveSettings({
              ...settings,
              window: {
                ...settings.window,
                width: size.width,
                height: size.height,
                x: position.x,
                y: position.y,
                isMaximized: false,
              },
            });
          }
        } catch (e) {
          console.error("Failed to save window state on close:", e);
        }
      });
    };

    setupCloseListener();

    return () => {
      unlistenClose?.();
    };
  }, []);

  const handleImportData = useCallback(() => {
    triggerImportData();
  }, []);

  const handleSplitRight = useCallback(() => {
    if (activeEditorGroupId) {
      const node = useLayoutStore.getState().nodes[activeEditorGroupId];
      const activeTab = node?.data?.tabs?.find((t) => t.id === node.data?.activeTabId);
      splitNode(activeEditorGroupId, "row", activeTab?.component || "GraphEditor");
    }
  }, [activeEditorGroupId, splitNode]);

  const handleSplitDown = useCallback(() => {
    if (activeEditorGroupId) {
      const node = useLayoutStore.getState().nodes[activeEditorGroupId];
      const activeTab = node?.data?.tabs?.find((t) => t.id === node.data?.activeTabId);
      splitNode(activeEditorGroupId, "col", activeTab?.component || "GraphEditor");
    }
  }, [activeEditorGroupId, splitNode]);

  const handleDataView = useCallback(async () => {
    try {
      const label = `dataview-${Math.random().toString(36).substring(7)}`;
      new WebviewWindow(label, {
        url: "index.html#/dataview",
        title: "Data Viewer",
        width: 1000,
        height: 600,
        decorations: false,
        visible: false,
      });
    } catch (error) {
      console.error("Failed to open data view:", error);
      uiStore.showToast("无法打开数据视图窗口", "error");
    }
  }, []);

  const handleOpenLogs = useCallback(async () => {
    try {
      const label = `logs-${Math.random().toString(36).substring(7)}`;
      new WebviewWindow(label, {
        url: "index.html#/logs",
        title: "Logs",
        width: 1000,
        height: 600,
        decorations: false,
        visible: false,
      });
    } catch (error) {
      console.error("Failed to open logs window:", error);
      uiStore.showToast("无法打开日志窗口", "error");
    }
  }, []);

  const toggleDetail = useCallback(() => {
    updateNode("detail", {
      data: { ...detailNode?.data, visible: !isDetailVisible },
    });
  }, [detailNode, isDetailVisible, updateNode]);

  const openNewWindow = useCallback(async () => {
    try {
      const label = `window-${Math.random().toString(36).substring(7)}`;
      new WebviewWindow(label, {
        url: "index.html",
        title: "YssBI Node Editor",
        width: 1000,
        height: 800,
        decorations: false,
      });
    } catch (error) {
      console.error("Failed to open new window:", error);
    }
  }, []);

  return {
    openSettings,
    isDetailVisible,
    handleImportData,
    handleSplitRight,
    handleSplitDown,
    handleDataView,
    handleOpenLogs,
    toggleDetail,
    openNewWindow,
  };
}
