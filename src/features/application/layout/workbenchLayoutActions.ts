import i18n from 'i18next';

import { requestCloseWorkbenchPanel } from '@/features/application/editor/workbenchPanelClose';
import { logsDockviewLayoutController } from '@/features/core/dockview/logsDockviewLayoutController';
import {
  orderWorkbenchPanelIdsForReset,
  WORKBENCH_EDGE_SIZES,
} from '@/features/core/dockview/workbenchDockviewDefaults';
import { workbenchDockviewInternal } from '@/features/core/dockview/workbenchDockviewInternal';
import {
  workbenchDockviewPort,
  type WorkbenchPanelInfo,
} from '@/features/core/dockview/workbenchDockviewPort';
import type { WorkbenchViewId } from '@/features/core/dockview/workbenchPanelModel';
import { useEditorStore } from '@/features/core/editor';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { workbenchLayoutController } from './workbenchLayoutController';
import { showWorkbenchLayoutError } from './workbenchLayoutErrorFeedback';

const VIEW_TITLE_KEYS = {
  resources: 'panel.resources',
  details: 'panel.details',
  inspect: 'panel.inspect',
  logs: 'panel.logs',
  output: 'panel.output',
} as const satisfies Record<WorkbenchViewId, string>;

function findWorkbenchView(viewId: WorkbenchViewId): WorkbenchPanelInfo | undefined {
  return workbenchDockviewPort.listPanels().find((panel) =>
    panel.metadata.role === 'view' && panel.metadata.viewId === viewId);
}

function hasContextFor(viewId: WorkbenchViewId): boolean {
  const focus = useEditorStore.getState().detailFocus;
  if (viewId === 'details') return focus !== null;
  if (viewId === 'inspect') return focus?.kind === 'node';
  return true;
}

function viewRequest(viewId: WorkbenchViewId) {
  return {
    viewId,
    title: i18n.t(VIEW_TITLE_KEYS[viewId]),
  };
}

export async function revealWorkbenchView(
  viewId: WorkbenchViewId,
): Promise<WorkbenchPanelInfo | null> {
  try {
    const existing = findWorkbenchView(viewId);
    if (existing) {
      return await workbenchDockviewPort.reveal(existing.panelInstanceId)
        ? existing
        : null;
    }
    if (!hasContextFor(viewId)) return null;
    return await workbenchDockviewPort.ensureView(viewRequest(viewId));
  } catch (error) {
    showWorkbenchLayoutError(error);
    return null;
  }
}

export async function toggleWorkbenchView(viewId: WorkbenchViewId): Promise<boolean> {
  const existing = findWorkbenchView(viewId);
  if (existing) return requestCloseWorkbenchPanel(existing.panelInstanceId);
  return (await revealWorkbenchView(viewId)) !== null;
}

export async function toggleBottomWorkbenchGroup(): Promise<void> {
  try {
    const bottom = workbenchDockviewPort.getEdgeState('bottom');
    if (bottom.exists
      && bottom.groupId
      && workbenchDockviewPort.listGroupPanels(bottom.groupId).length > 0) {
      await workbenchDockviewPort.setEdgeCollapsed('bottom', !bottom.collapsed);
      return;
    }

    const logs = findWorkbenchView('logs');
    if (logs) {
      await workbenchDockviewPort.reveal(logs.panelInstanceId);
      return;
    }
  } catch (error) {
    showWorkbenchLayoutError(error);
    return;
  }

  await revealWorkbenchView('logs');
}

export async function resetWorkbenchLayout(): Promise<void> {
  const resetEpoch = workbenchLayoutController.beginLayoutReset();
  try {
    await workbenchDockviewInternal.runLayoutTransaction((tx) => {
      const before = tx.listPanels();
      const beforeById = new Map(
        before.map((panel) => [panel.panelInstanceId, panel] as const),
      );
      const ordered = orderWorkbenchPanelIdsForReset(
        tx.serialize(),
        before.map((panel) => panel.panelInstanceId),
      ).map((panelId) => beforeById.get(panelId)!);

      const physicallyActive = tx.getActivePanel();
      const focused = useGraphSessionStore.getState().focusedSession;
      const editorToRestore = (
        physicallyActive?.metadata.role === 'editor'
          ? physicallyActive
          : focused
            ? before.find((panel) =>
                panel.metadata.role === 'editor'
                && panel.groupId === focused.groupId
                && panel.metadata.resourceRef === focused.graphPath)
            : undefined
      ) ?? ordered.find((panel) => panel.metadata.role === 'editor');

      const editors = ordered.filter((panel) => panel.metadata.role === 'editor');
      const resources = tx.ensureView(viewRequest('resources'));
      const logs = tx.ensureView(viewRequest('logs'));
      const output = tx.ensureView(viewRequest('output'));
      const left = tx.configureEdge({
        position: 'left',
        size: WORKBENCH_EDGE_SIZES.left,
        collapsed: false,
      });
      const bottom = tx.configureEdge({
        position: 'bottom',
        size: WORKBENCH_EDGE_SIZES.bottom,
        collapsed: false,
        headerPosition: 'bottom',
      });
      const centralGroupId = tx.ensureCentralGroup();

      const firstEditor = editors[0];
      if (firstEditor) {
        tx.move({
          panelInstanceId: firstEditor.panelInstanceId,
          groupId: centralGroupId,
          index: 0,
        });
      }

      tx.move({
        panelInstanceId: resources.panelInstanceId,
        groupId: left.groupId,
        index: 0,
      });
      tx.move({
        panelInstanceId: logs.panelInstanceId,
        groupId: bottom.groupId,
        index: 0,
      });
      tx.move({
        panelInstanceId: output.panelInstanceId,
        groupId: bottom.groupId,
        index: 1,
      });

      for (const [offset, panel] of editors.slice(1).entries()) {
        tx.move({
          panelInstanceId: panel.panelInstanceId,
          groupId: centralGroupId,
          index: offset + 1,
        });
      }

      const contextual = ordered.filter((panel) =>
        panel.metadata.role === 'result'
        || (panel.metadata.role === 'view'
          && (panel.metadata.viewId === 'details'
            || panel.metadata.viewId === 'inspect')));
      if (contextual.length > 0) {
        const right = tx.configureEdge({
          position: 'right',
          size: WORKBENCH_EDGE_SIZES.right,
          collapsed: false,
        });
        for (const [index, panel] of contextual.entries()) {
          tx.move({
            panelInstanceId: panel.panelInstanceId,
            groupId: right.groupId,
            index,
          });
        }
      }

      tx.activate(editorToRestore?.panelInstanceId ?? resources.panelInstanceId);
    });
    logsDockviewLayoutController.resetToDefault();
  } catch (error) {
    showWorkbenchLayoutError(error);
  } finally {
    workbenchLayoutController.completeLayoutReset(resetEpoch);
  }
}
