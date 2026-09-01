import { workbenchDockviewRead } from "@/features/core/dockview/workbenchRead";

export function isGraphOpenInAnyEditorPanel(graphPath: string): boolean {
  return workbenchDockviewRead.findEditorPanelsByResource(graphPath).length > 0;
}
