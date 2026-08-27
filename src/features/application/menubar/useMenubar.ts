import { useCallback, useMemo, useSyncExternalStore } from 'react';
import { triggerImportData } from '@/features/application/dataManagement/useDatabaseManagement';
import {
  captureActiveEditorCommandTarget,
} from '@/features/application/editor/editorCommandFocus';
import { splitEditorAtEdge } from '@/features/application/editor/editorGroupCommands';
import {
  resetWorkbenchLayout,
  toggleActivityWorkbenchGroup,
  toggleWorkbenchView,
} from '@/features/application/layout/workbenchLayoutActions';
import {
  openDatabaseEditorWindow,
  openLogsWindow,
} from '@/features/application/window';

import { workbenchDockviewPort } from '@/features/core/dockview/workbenchDockviewPort';
import type { WorkbenchViewId } from '@/features/core/dockview/workbenchPanelModel';
import { useEditorStore } from '@/features/core/editor/stores/useEditorStore';
import { useWorkbenchStore } from '@/features/core/workbench/workbenchStore';
import type { MenubarViewState } from './menubarViewItems';

function openViewIds(): ReadonlySet<WorkbenchViewId> {
  const viewIds = new Set<WorkbenchViewId>();
  for (const panel of workbenchDockviewPort.listPanels()) {
    if (panel.metadata.role === 'view') viewIds.add(panel.metadata.viewId);
  }
  return viewIds;
}

/** Menubar model projected from live root Dockview state and semantic application actions. */
export function useMenubar() {
  const openSettings = useWorkbenchStore((state) => state.openSettings);
  const inspectContextValid = useEditorStore((state) => state.detailFocus?.kind === 'node');
  const dockviewSnapshot = useSyncExternalStore(
    workbenchDockviewPort.subscribe,
    workbenchDockviewPort.getSnapshot,
    workbenchDockviewPort.getSnapshot,
  );

  const viewState = useMemo<MenubarViewState>(() => {
    const views = openViewIds();
    return {
      activityGroupOpen: (() => {
        const edge = workbenchDockviewPort.getEdgeState('left');
        return edge.exists && edge.visible && !edge.collapsed
          && edge.groupId === 'workbench-edge-left';
      })(),
      assistantOpen: views.has('assistant'),
      inspectOpen: views.has('inspect'),
      inspectContextValid,
      logsOpen: views.has('logs'),
      outputOpen: views.has('output'),
      bottomCollapsed: workbenchDockviewPort.getEdgeState('bottom').collapsed,
    };
  }, [dockviewSnapshot.revision, inspectContextValid]);

  const editorCommandAuthorized = captureActiveEditorCommandTarget() !== null;

  const handleImportData = useCallback(() => {
    triggerImportData();
  }, []);

  const handleSplitRight = useCallback(() => {
    const target = captureActiveEditorCommandTarget();
    if (target) void splitEditorAtEdge(target.groupId, 'right');
  }, []);

  const handleSplitDown = useCallback(() => {
    const target = captureActiveEditorCommandTarget();
    if (target) void splitEditorAtEdge(target.groupId, 'bottom');
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
    void toggleWorkbenchView('assistant');
  }, []);

  const toggleInspect = useCallback(() => {
    void toggleWorkbenchView('inspect');
  }, []);

  const toggleLogs = useCallback(() => {
    void toggleWorkbenchView('logs');
  }, []);

  const toggleOutput = useCallback(() => {
    void toggleWorkbenchView('output');
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
      toggleInspect,
      toggleLogs,
      toggleOutput,
      resetLayout: handleResetLayout,
    },
  };
}
