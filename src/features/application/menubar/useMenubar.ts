import { useCallback } from "react";
import {
  editorDockviewPort,
  panelDockviewPort,
  useDockviewPortSnapshot,
} from '@/features/core/dockview';
import { useWorkbenchStore } from '@/features/core/workbench';
import {
  resetWorkbenchLayout,
  toggleDetailVisibility,
  togglePanelCollapsed,
  toggleSidebarVisibility,
} from "@/features/core/layout/workbenchLayoutService";
import { normalizePanelPosition } from "@/features/core/layout/panelPartLayout";
import { useSettingsStore } from "@/features/core/settings/settingsStore";
import { splitEditorAtEdge } from "@/features/application/editor/editorGroupCommands";
import { triggerImportData } from "@/features/application/dataManagement/useDatabaseManagement";
import {
  openBayesWindow,
  openDatabaseEditorWindow,
  openLogsWindow,
} from "@/features/application/window";

/** Menubar model for menu state and menu-triggered application commands. */
export function useMenubar() {
  const openSettings = useWorkbenchStore((state) => state.openSettings);
  const isDetailVisible = useWorkbenchStore((state) => !state.detailUserHidden);
  const isSidebarVisible = useWorkbenchStore((state) => !state.sidebarUserHidden);
  const panelCollapsed = useDockviewPortSnapshot(panelDockviewPort).collapsed ?? false;
  const isLogPanelVisible = !panelCollapsed;
  useDockviewPortSnapshot(editorDockviewPort);
  const activeEditorGroupId = editorDockviewPort.getActiveGroupId() ?? null;

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
    void resetWorkbenchLayout(panelPosition);
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
    toggleLogPanel: togglePanelCollapsed,
    toggleSidebar: toggleSidebarVisibility,
    handleResetLayout,
  };
}
