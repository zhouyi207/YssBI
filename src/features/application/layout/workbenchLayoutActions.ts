import i18n from 'i18next';

import { requestCloseWorkbenchPanel } from '@/features/application/editor/workbenchPanelClose';
import { logsDockviewControl } from '@/features/core/dockview/logsControl';
import {
  orderWorkbenchPanelIdsForReset,
  WORKBENCH_ACTIVITY_DEFAULT_ORDER,
  WORKBENCH_EDGE_SIZES,
} from '@/features/core/dockview/workbenchDockviewDefaults';
import { workbenchDockviewInternal } from '@/features/core/dockview/workbenchDockviewInternal';
import {
  workbenchDockviewRead,
  type WorkbenchPanelInfo,
} from '@/features/core/dockview/workbenchRead';
import { workbenchDockviewControl } from '@/features/core/dockview/workbenchControl';
import {
  isWorkbenchActivityMetadata,
  isWorkbenchPersistentViewMetadata,
  type WorkbenchViewId,
} from '@/features/core/dockview/workbenchPanelModel';
import { useEditorStore } from '@/features/core/editor';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { workbenchLayoutController } from './workbenchLayoutController';
import { showWorkbenchLayoutError } from './workbenchLayoutErrorFeedback';

const VIEW_TITLE_KEYS = {
  project: 'activityBar.project',
  nodes: 'activityBar.nodes',
  data: 'activityBar.data',
  commands: 'activityBar.commands',
  details: 'panel.details',
  inspect: 'panel.inspect',
  logs: 'panel.logs',
  output: 'panel.output',
  diagnostics: 'panel.diagnostics',
} as const satisfies Record<WorkbenchViewId, string>;

function findWorkbenchView(viewId: WorkbenchViewId): WorkbenchPanelInfo | undefined {
  return workbenchDockviewRead.listPanels().find((panel) =>
    panel.metadata.role === 'view' && panel.metadata.viewId === viewId);
}

function hasContextFor(viewId: WorkbenchViewId): boolean {
  const focus = useEditorStore.getState().detailFocus;
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
      return await workbenchDockviewControl.reveal(existing.panelInstanceId)
        ? existing
        : null;
    }
    if (!hasContextFor(viewId)) return null;
    return await workbenchDockviewControl.ensureView(viewRequest(viewId));
  } catch (error) {
    showWorkbenchLayoutError(error);
    return null;
  }
}

export async function toggleWorkbenchView(viewId: WorkbenchViewId): Promise<boolean> {
  const existing = findWorkbenchView(viewId);
  if (existing && (
    isWorkbenchActivityMetadata(existing.metadata)
    || isWorkbenchPersistentViewMetadata(existing.metadata)
  )) {
    return (await revealWorkbenchView(viewId)) !== null;
  }
  if (existing) return requestCloseWorkbenchPanel(existing.panelInstanceId);
  return (await revealWorkbenchView(viewId)) !== null;
}

function activityPanelsInGroup(groupId: string): boolean {
  const panels = workbenchDockviewRead.listGroupPanels(groupId);
  return panels.length === WORKBENCH_ACTIVITY_DEFAULT_ORDER.length
    && panels.every((panel) => isWorkbenchActivityMetadata(panel.metadata));
}

async function ensureActivityWorkbenchGroup(): Promise<void> {
  await workbenchDockviewInternal.runLayoutTransaction((tx) => {
    const panels = WORKBENCH_ACTIVITY_DEFAULT_ORDER.map((viewId) =>
      tx.ensureView(viewRequest(viewId)));
    const left = tx.configureEdge({
      position: 'left',
      size: WORKBENCH_EDGE_SIZES.left,
      collapsed: false,
      headerPosition: 'left',
    });
    panels.forEach((panel, index) => {
      tx.move({ panelInstanceId: panel.panelInstanceId, groupId: left.groupId, index });
    });
    const project = panels.find((panel) =>
      panel.metadata.role === 'view' && panel.metadata.viewId === 'project');
    if (project) tx.activate(project.panelInstanceId);
  });
}

export async function toggleActivityWorkbenchGroup(): Promise<void> {
  try {
    const left = workbenchDockviewRead.getEdgeState('left');
    if (left.exists && left.groupId && activityPanelsInGroup(left.groupId)) {
      await workbenchDockviewControl.setEdgeCollapsed('left', left.visible && !left.collapsed);
      return;
    }
    await ensureActivityWorkbenchGroup();
  } catch (error) {
    showWorkbenchLayoutError(error);
  }
}

export async function toggleBottomWorkbenchGroup(): Promise<void> {
  try {
    const bottom = workbenchDockviewRead.getEdgeState('bottom');
    if (bottom.exists
      && bottom.groupId
      && workbenchDockviewRead.listGroupPanels(bottom.groupId).length > 0) {
      await workbenchDockviewControl.setEdgeCollapsed('bottom', !bottom.collapsed);
      return;
    }

    const logs = findWorkbenchView('logs');
    if (logs) {
      await workbenchDockviewControl.reveal(logs.panelInstanceId);
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
      const activityPanels = WORKBENCH_ACTIVITY_DEFAULT_ORDER.map((viewId) =>
        tx.ensureView(viewRequest(viewId)));
      const details = tx.ensureView(viewRequest('details'));
      const logs = tx.ensureView(viewRequest('logs'));
      const output = tx.ensureView(viewRequest('output'));
      const diagnostics = tx.ensureView(viewRequest('diagnostics'));
      const left = tx.configureEdge({
        position: 'left',
        size: WORKBENCH_EDGE_SIZES.left,
        collapsed: false,
        headerPosition: 'left',
      });
      const right = tx.configureEdge({
        position: 'right',
        size: WORKBENCH_EDGE_SIZES.right,
        collapsed: false,
        headerPosition: 'right',
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

      activityPanels.forEach((panel, index) => {
        tx.move({
          panelInstanceId: panel.panelInstanceId,
          groupId: left.groupId,
          index,
        });
      });
      tx.move({
        panelInstanceId: details.panelInstanceId,
        groupId: right.groupId,
        index: 0,
        activate: false,
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
      tx.move({
        panelInstanceId: diagnostics.panelInstanceId,
        groupId: bottom.groupId,
        index: 2,
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
          && panel.metadata.viewId === 'inspect'));
      for (const [index, panel] of contextual.entries()) {
        tx.move({
          panelInstanceId: panel.panelInstanceId,
          groupId: right.groupId,
          index: index + 1,
        });
      }

      const project = activityPanels.find((panel) =>
        panel.metadata.role === 'view' && panel.metadata.viewId === 'project');
      tx.activate(editorToRestore?.panelInstanceId ?? project?.panelInstanceId ?? '');
    });
    logsDockviewControl.resetToDefault();
  } catch (error) {
    showWorkbenchLayoutError(error);
  } finally {
    workbenchLayoutController.completeLayoutReset(resetEpoch);
  }
}
