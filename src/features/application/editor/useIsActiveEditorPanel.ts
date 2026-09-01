import { workbenchDockviewRead } from "@/modules/workbench/public";
import { useDockviewPortSnapshot } from "@/modules/workbench/public";

/** True only for the physically active editor panel. */
export function useIsActiveEditorPanel(panelInstanceId?: string | null): boolean {
  useDockviewPortSnapshot(workbenchDockviewRead);
  return (
    panelInstanceId != null &&
    workbenchDockviewRead.getActiveEditorPanel()?.panelInstanceId === panelInstanceId
  );
}
