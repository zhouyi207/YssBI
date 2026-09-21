import { useCallback, useMemo, useSyncExternalStore } from "react";
import { triggerImportData } from "@/features/application/dataManagement/useDatabaseManagement";
import { captureActiveEditorCommandTarget } from "@/features/application/editor/editorCommandFocus";
import { splitEditorPanel } from "@/features/application/editor/editorGroupCommands";
import {
  resetWorkbenchLayout,
  toggleActivityWorkbenchGroup,
  toggleWorkbenchView,
  WORKBENCH_ACTIVITY_GROUP_ID,
} from "@/modules/workbench/public";
import { openDatabaseEditorWindow, openLogsWindow } from "@/features/application/window";

import { workbenchLayoutRead } from "@/modules/workbench/public";
import type { WorkbenchViewId } from "@/modules/workbench/public";
import { useWorkbenchUiStore } from "@/modules/workbench/public";
import type { MenubarViewState } from "./menubarViewItems";

function openViewIds(): ReadonlySet<WorkbenchViewId> {
  const viewIds = new Set<WorkbenchViewId>();
  for (const panel of workbenchLayoutRead.listPanels()) {
    if (panel.metadata.role === "view") viewIds.add(panel.metadata.viewId);
  }
  return viewIds;
}

/** Menubar model projected from live root FlexLayout state and semantic application actions. */
export function useMenubar() {
  const openSettings = useWorkbenchUiStore((state) => state.openSettings);
  const flexlayoutSnapshot = useSyncExternalStore(
    workbenchLayoutRead.subscribe,
    workbenchLayoutRead.getSnapshot,
    workbenchLayoutRead.getSnapshot,
  );

  const viewState = useMemo<MenubarViewState>(() => {
    const views = openViewIds();
    return {
      activityGroupOpen: (() => {
        const edge = workbenchLayoutRead.getEdgeState("left");
        return (
          edge.exists &&
          edge.visible &&
          !edge.collapsed &&
          edge.groupId === WORKBENCH_ACTIVITY_GROUP_ID
        );
      })(),
      assistantOpen: views.has("assistant"),
    };
  }, [flexlayoutSnapshot.revision]);

  const editorCommandAuthorized = captureActiveEditorCommandTarget() !== null;

  const handleImportData = useCallback(() => {
    triggerImportData();
  }, []);

  const handleSplitRight = useCallback(() => {
    const target = captureActiveEditorCommandTarget();
    if (target) void splitEditorPanel(target.groupId, "right");
  }, []);

  const handleSplitDown = useCallback(() => {
    const target = captureActiveEditorCommandTarget();
    if (target) void splitEditorPanel(target.groupId, "bottom");
  }, []);

  const handleDatabaseEditor = useCallback(() => {
    void openDatabaseEditorWindow();
  }, []);

  const handleOpenLogs = useCallback(() => {
    void openLogsWindow();
  }, []);

  const toggleActivityGroup = useCallback(() => {
    void toggleActivityWorkbenchGroup();
  }, []);

  const toggleAssistant = useCallback(() => {
    void toggleWorkbenchView("assistant");
  }, []);

  const handleResetLayout = useCallback(() => {
    void resetWorkbenchLayout();
  }, []);

  return {
    openSettings,
    editorCommandAuthorized,
    viewState,
    handleImportData,
    handleSplitRight,
    handleSplitDown,
    handleDatabaseEditor,
    handleOpenLogs,
    viewActions: {
      toggleActivityGroup,
      toggleAssistant,
      resetLayout: handleResetLayout,
    },
  };
}
