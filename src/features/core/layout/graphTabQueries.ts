import { workbenchDockviewPort } from '@/features/core/dockview/workbenchDockviewPort';

export function isGraphOpenInAnyTab(graphPath: string): boolean {
  return workbenchDockviewPort.findEditorPanelsByResource(graphPath).length > 0;
}
