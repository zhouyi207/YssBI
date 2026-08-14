import { editorDockviewPort } from '@/features/core/dockview';

export function isGraphOpenInAnyTab(graphPath: string): boolean {
  return editorDockviewPort.findPanelsByResource(graphPath).length > 0;
}
