import { ProjectLifecycleProtocolError } from '@/features/application/projectLifecycleReceipt';
import { workbenchLayoutController } from '@/features/application/layout/workbenchLayoutController';
import { resetProjectScopedRightSidebarState } from '@/features/application/project/projectReset';
import { useProjectIOStore } from '@/features/application/project/projectIOStore';
import { useEditorPaneStateStore } from '@/features/core/dockview/editorPaneStateStore';
import { workbenchDockviewInternal } from '@/features/core/dockview/workbenchDockviewInternal';
import { workbenchDockviewRead } from '@/features/core/dockview/workbenchRead';
import type { WorkbenchPanelInfo } from '@/features/core/dockview/workbenchRead';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import {
  isProjectLifecycleStateCurrent,
  type ProjectLifecycleStateSnapshot,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';

function ownsPreviousProject(
  previousProjectInstanceId: string,
  owner: ProjectLifecycleStateSnapshot,
): boolean {
  if (!isProjectLifecycleStateCurrent(owner)) return false;
  if (useProjectIOStore.getState().projectInstanceId !== previousProjectInstanceId) {
    throw new ProjectLifecycleProtocolError('stale project cleanup', true);
  }
  return true;
}

function isProjectScopedPanel(panel: WorkbenchPanelInfo): boolean {
  if (panel.metadata.role === 'editor' || panel.metadata.role === 'result') return true;
  return panel.metadata.role === 'view'
    && panel.metadata.viewId === 'inspect';
}

export async function removeProjectScopedWorkbenchPanels(
  previousProjectInstanceId: string,
  owner: ProjectLifecycleStateSnapshot,
): Promise<void> {
  if (!ownsPreviousProject(previousProjectInstanceId, owner)) return;
  workbenchLayoutController.invalidateForProjectReplacement();
  if (!ownsPreviousProject(previousProjectInstanceId, owner)) return;

  if (workbenchDockviewRead.isReady) {
    if (!ownsPreviousProject(previousProjectInstanceId, owner)) return;
    await workbenchDockviewInternal.runLayoutTransaction((transaction) => {
      if (!ownsPreviousProject(previousProjectInstanceId, owner)) return;
      const panels = transaction.listPanels().filter(isProjectScopedPanel);
      if (!ownsPreviousProject(previousProjectInstanceId, owner)) return;
      transaction.removePanels(panels.map((panel) => panel.panelInstanceId));
    });
  }

  if (!ownsPreviousProject(previousProjectInstanceId, owner)) return;
  useEditorPaneStateStore.getState().reset();
  if (!ownsPreviousProject(previousProjectInstanceId, owner)) return;
  useGraphSessionStore.getState().reset();
  if (!ownsPreviousProject(previousProjectInstanceId, owner)) return;
  resetProjectScopedRightSidebarState();
  ownsPreviousProject(previousProjectInstanceId, owner);
}
