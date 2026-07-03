import { useEffect, useCallback } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { collectDirtyGraphTabs } from "@/features/core/layout/tabDirty";
import { saveAllDirtyGraphs } from "@/features/application/editor/saveAllDirtyGraphs";
import { triggerImportData } from "@/features/application/dataManagement/useDatabaseManagement";
import { createPersistedWindow } from "@/features/application/window";
import { uiStore } from "@/features/core/ui/UIStore";
import { i18n } from "@/app/i18n";
import { logger } from '@/utils/appLogger';

/**
 * Menubar logic: window lifecycle, import, split, open windows, etc.
 * Extracted from Menubar.tsx - view should only consume this hook.
 *
 * 窗口几何状态（位置/大小/最大化）由 `usePersistedWindow("main")` 在
 * EditorWindow / ProjectPickerScreen 等主窗口入口处统一接入；这里仅负责
 * 拦截关闭以处理脏标签，不再自行写入 settings。
 */
export function useMenubar() {
  const openSettings = useLayoutStore((s) => s.openSettings);
  const activeEditorGroupId = useLayoutStore((s) => s.activeEditorGroupId);
  const splitNode = useLayoutStore((s) => s.splitNode);
  const detailNode = useLayoutStore((s) => s.nodes["detail"]);
  const panelNode = useLayoutStore((s) => s.nodes["panel"]);
  const updateNode = useLayoutStore((s) => s.updateNode);

  const isDetailVisible = detailNode?.data?.visible !== false;
  const isLogPanelVisible = panelNode?.data?.visible !== false;

  useEffect(() => {
    const appWindow = getCurrentWindow();
    let unlistenClose: (() => void) | null = null;
    // Set after the user confirms a destructive close so the next close request
    // bypasses the dirty-tab gate. Tauri's `onCloseRequested` only suppresses
    // the close when `event.preventDefault()` is called, so re-issuing
    // `appWindow.close()` triggers this listener again.
    let allowDestructiveClose = false;

    const setupCloseListener = async () => {
      unlistenClose = await appWindow.onCloseRequested(async (event) => {
        if (allowDestructiveClose) return;

        const dirty = collectDirtyGraphTabs();
        if (dirty.length === 0) return;

        // Block the close until the user picks an action.
        event.preventDefault();

        const titles = dirty.map((d) => `• ${d.title}`).join("\n");
        const choice = await uiStore.confirm3({
          title: i18n.t("editor.unsavedTitle", { defaultValue: "保存更改？" }),
          message: i18n.t("editor.unsavedMessage", {
            defaultValue: `以下 {{count}} 个图存在未保存修改：\n{{titles}}\n\n关闭前是否保存？`,
            count: dirty.length,
            titles,
          }),
          confirmText: i18n.t("editor.unsavedSaveAll", { defaultValue: "全部保存" }),
          discardText: i18n.t("editor.unsavedDiscard", { defaultValue: "不保存" }),
          cancelText: i18n.t("common.cancel", { defaultValue: "取消" }),
          type: "info",
        });

        if (choice === "cancel") return;

        if (choice === "confirm") {
          const ok = await saveAllDirtyGraphs();
          if (!ok) return;
        }

        allowDestructiveClose = true;
        try {
          await appWindow.close();
        } catch (e) {
          logger.app.error(
            `Failed to close window after dirty-tab decision: ${e instanceof Error ? e.message : String(e)}`,
            'Menubar'
          );
          allowDestructiveClose = false;
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

  const handleDatabaseEditor = useCallback(async () => {
    try {
      const label = `dataview-${Math.random().toString(36).substring(7)}`;
      await createPersistedWindow({
        kind: "databaseEditor",
        label,
        url: "index.html#/database",
        title: i18n.t("databaseEditor.title"),
      });
    } catch (error) {
      logger.app.error(`Failed to open data view: ${error instanceof Error ? error.message : String(error)}`, 'Menubar');
      uiStore.showToast(i18n.t("databaseEditor.failedOpenWindow"), "error");
    }
  }, []);

  const handleOpenLogs = useCallback(async () => {
    try {
      const label = `logs-${Math.random().toString(36).substring(7)}`;
      await createPersistedWindow({
        kind: "logs",
        label,
        url: "index.html#/logs",
        title: "Logs",
      });
    } catch (error) {
      logger.app.error(`Failed to open logs window: ${error instanceof Error ? error.message : String(error)}`, 'Menubar');
      uiStore.showToast("无法打开日志窗口", "error");
    }
  }, []);

  const toggleDetail = useCallback(() => {
    updateNode("detail", {
      data: { ...detailNode?.data, visible: !isDetailVisible },
    });
  }, [detailNode, isDetailVisible, updateNode]);

  const toggleLogPanel = useCallback(() => {
    updateNode("panel", {
      data: { ...panelNode?.data, visible: !isLogPanelVisible },
    });
  }, [panelNode, isLogPanelVisible, updateNode]);

  const openNewWindow = useCallback(async () => {
    try {
      const label = `window-${Math.random().toString(36).substring(7)}`;
      await createPersistedWindow({
        kind: "main",
        label,
        url: "index.html",
        title: "YssBI Node Editor",
        visible: true,
      });
    } catch (error) {
      logger.app.error(`Failed to open new window: ${error instanceof Error ? error.message : String(error)}`, 'Menubar');
    }
  }, []);

  return {
    openSettings,
    isDetailVisible,
    isLogPanelVisible,
    handleImportData,
    handleSplitRight,
    handleSplitDown,
    handleDatabaseEditor,
    handleOpenLogs,
    toggleDetail,
    toggleLogPanel,
    openNewWindow,
  };
}
