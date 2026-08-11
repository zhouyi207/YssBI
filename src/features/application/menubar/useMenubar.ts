import { useEffect, useCallback } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useLayoutStore } from "@/features/core/layout/layoutStore";
import {
  resetWorkbenchLayout,
  toggleDetailVisibility,
  togglePanelVisibility,
  toggleSidebarVisibility,
} from "@/features/core/layout/workbenchLayoutService";
import { normalizePanelPosition } from "@/features/core/layout/panelPartLayout";
import { useSettingsStore } from "@/features/core/settings/settingsStore";
import { collectDirtyGraphTabs } from "@/features/core/layout/tabDirty";
import { saveAllDirtyGraphs } from "@/features/application/editor/saveAllDirtyGraphs";
import { splitEditorAtEdge } from "@/features/application/editor/editorGroupCommands";
import { triggerImportData } from "@/features/application/dataManagement/useDatabaseManagement";
import {
  openBayesWindow,
  openDatabaseEditorWindow,
  openLogsWindow,
} from "@/features/application/window";
import { uiStore } from "@/features/core/ui/UIStore";
import { i18n } from "@/app/i18n";
import { logger } from '@/utils/appLogger';

/**
 * Menubar logic: window lifecycle, import, split, open windows, etc.
 * Extracted from Menubar.tsx - view should only consume this hook.
 *
 * 窗口几何状态（位置/大小/最大化）由 `useEditorWindowGeometryPersistence` 在
 * EditorWindow / ProjectPickerScreen 等主窗口入口处统一接入；这里仅负责
 * 拦截关闭以处理脏标签，不再自行写入 settings。
 */
export function useMenubar() {
  const openSettings = useLayoutStore((s) => s.openSettings);
  const activeEditorGroupId = useLayoutStore((s) => s.activeEditorGroupId);
  const detailNode = useLayoutStore((s) => s.nodes["detail"]);
  const panelNode = useLayoutStore((s) => s.nodes["panel"]);
  const sidebarNode = useLayoutStore((s) => s.nodes["sidebar"]);

  const isDetailVisible = detailNode?.data?.visible !== false;
  const isLogPanelVisible = panelNode?.data?.visible !== false;
  const isSidebarVisible = sidebarNode?.data?.visible !== false;

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
      void splitEditorAtEdge(activeEditorGroupId, "right");
    }
  }, [activeEditorGroupId]);

  const handleSplitDown = useCallback(() => {
    if (activeEditorGroupId) {
      void splitEditorAtEdge(activeEditorGroupId, "bottom");
    }
  }, [activeEditorGroupId]);

  const handleDatabaseEditor = useCallback(() => {
    void openDatabaseEditorWindow();
  }, []);

  const handleOpenLogs = useCallback(() => {
    void openLogsWindow();
  }, []);

  const handleOpenBayes = useCallback(() => {
    void openBayesWindow();
  }, []);

  const handleResetLayout = useCallback(() => {
    const panelPosition = normalizePanelPosition(
      useSettingsStore.getState().appearance.panelPosition,
    );
    resetWorkbenchLayout(panelPosition);
  }, []);

  return {
    openSettings,
    isDetailVisible,
    isLogPanelVisible,
    isSidebarVisible,
    handleImportData,
    handleSplitRight,
    handleSplitDown,
    handleDatabaseEditor,
    handleOpenLogs,
    handleOpenBayes,
    toggleDetail: toggleDetailVisibility,
    toggleLogPanel: togglePanelVisibility,
    toggleSidebar: toggleSidebarVisibility,
    handleResetLayout,
  };
}
