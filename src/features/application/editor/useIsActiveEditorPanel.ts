import { workbenchDockviewPort } from '@/features/core/dockview/workbenchDockviewPort';
import { useDockviewPortSnapshot } from '@/features/core/dockview/useDockviewPortSnapshot';

/** True only for the physically active editor panel. */
export function useIsActiveEditorPanel(panelInstanceId?: string | null): boolean {
  useDockviewPortSnapshot(workbenchDockviewPort);
  return panelInstanceId != null
    && workbenchDockviewPort.getActiveEditorPanel()?.panelInstanceId === panelInstanceId;
}
