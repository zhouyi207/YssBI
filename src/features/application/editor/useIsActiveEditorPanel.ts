import { workbenchDockviewRead } from "@/features/core/dockview/workbenchRead";
import { useDockviewPortSnapshot } from "@/features/core/dockview/useDockviewPortSnapshot";

/** True only for the physically active editor panel. */
export function useIsActiveEditorPanel(panelInstanceId?: string | null): boolean {
  useDockviewPortSnapshot(workbenchDockviewRead);
  return (
    panelInstanceId != null &&
    workbenchDockviewRead.getActiveEditorPanel()?.panelInstanceId === panelInstanceId
  );
}
