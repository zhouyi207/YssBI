import { workbenchDockviewRead } from "@/modules/workbench/public";

export function isGraphOpenInAnyEditorPanel(graphPath: string): boolean {
  return workbenchDockviewRead.findEditorPanelsByResource(graphPath).length > 0;
}
