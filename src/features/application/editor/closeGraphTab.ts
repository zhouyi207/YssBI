import i18n from 'i18next';
import type { LayoutTab } from '@/shared/types';
import { editorDockviewPort } from '@/features/core/dockview';
import { uiStore } from '@/features/core/ui/UIStore';
import { GraphService } from '@/services/graph/graphService';
import { closeGraphDocumentPanel } from './graphDocumentCloseLifecycle';
import { resolveTabDisplayName } from './resolveTabDisplayName';
import { clearDetailFocusForClosedTab } from '@/features/core/editor/detail/clearDetailFocusForClosedTab';
import { focusDetailOnActiveGraph } from '@/features/core/editor/detail/detailFocusCommands';
import { syncVariablesGraphScopeAfterClose } from '@/features/core/editor/detail/variablesGraphScope';
import { useEditorStore } from '@/features/core/editor/stores/useEditorStore';
import { isGraphResourceDirty, markResourceDirty } from '@/features/core/resource';
import { releaseEditorViewport } from '@/features/core/viewport';
import { editorViewportScope } from '@/features/core/viewport/viewportScope';
import { switchEditorTab } from './switchEditorTab';
import { deactivateGraphTab } from './activateGraphTab';
import { showBlockingIpcError } from './blockingErrorDialog';
import {
  captureSettledGraphSaveCommandContext,
  isGraphSaveCommandRevisionCurrent,
  type GraphSaveCommandContext,
} from '@/features/application/projectCommandContext';

function readLayoutTab(panel: ReturnType<typeof editorDockviewPort.listPanels>[number]): LayoutTab | null {
  const value = panel.tab?.data?.layoutTab;
  return value && typeof value === 'object' ? value as unknown as LayoutTab : null;
}

export async function closeGraphTab(graphPath: string, groupId?: string, skipDirtyPrompt = false): Promise<boolean> {
  const panel = editorDockviewPort
    .findPanelsByResource(graphPath)
    .find((candidate) => !groupId || candidate.groupId === groupId);
  const tab = panel ? readLayoutTab(panel) : null;
  if (!panel || !tab || (tab.type !== 'event' && tab.type !== 'function')) return false;

  if (isGraphResourceDirty(graphPath, tab.type) && !skipDirtyPrompt) {
    const shouldSave = await uiStore.confirm({
      title: '保存更改？',
      message: `“${resolveTabDisplayName({ id: graphPath, kind: tab.type }, graphPath)}” 已修改。关闭前是否保存？`,
      confirmText: '保存',
      cancelText: '不保存',
      type: 'info',
    });
    if (shouldSave) {
      let context: GraphSaveCommandContext | undefined;
      try {
        context = await captureSettledGraphSaveCommandContext(graphPath);
        await GraphService.saveProjectGraph(
          context.projectInstanceId,
          graphPath,
          context.expectedRevision,
          context.operationId,
        );
        if (!isGraphSaveCommandRevisionCurrent(context, graphPath)) return false;
        markResourceDirty({ id: graphPath, kind: tab.type }, false);
      } catch (error) {
        if (context && !context.isCurrent()) return false;
        showBlockingIpcError(error, 'save_project_graph', (code) =>
          i18n.t('notifications.editor.graphSaveFailed', { error: code }));
        return false;
      }
    }
  }

  const wasActive = editorDockviewPort.getActivePanel()?.panelInstanceId === panel.panelInstanceId;
  await closeGraphDocumentPanel({
    graphPath,
    graphKind: tab.type,
    panelInstanceId: panel.panelInstanceId,
    afterPanelRemoved: async () => {
      releaseEditorViewport(editorViewportScope(panel.groupId, graphPath));
      if (wasActive) deactivateGraphTab(panel.groupId, graphPath);
      clearDetailFocusForClosedTab(graphPath);
      syncVariablesGraphScopeAfterClose(graphPath);

      const active = editorDockviewPort.getActivePanel();
      const activeTab = active ? readLayoutTab(active) : null;
      if (active && activeTab) await switchEditorTab(active.groupId, activeTab);
      if (!useEditorStore.getState().detailFocus) focusDetailOnActiveGraph(active?.groupId ?? panel.groupId);
    },
  });
  return true;
}
