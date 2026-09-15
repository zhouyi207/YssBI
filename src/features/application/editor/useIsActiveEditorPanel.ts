import { workbenchLayoutRead } from "@/modules/workbench/public";
import { useLayoutPortSnapshot } from "@/modules/workbench/public";

/** True only for the physically active editor panel. */
export function useIsActiveEditorPanel(panelInstanceId?: string | null): boolean {
  return useLayoutPortSnapshot(
    workbenchLayoutRead,
    () =>
      panelInstanceId != null &&
      workbenchLayoutRead.getActiveEditorPanel()?.panelInstanceId === panelInstanceId,
  );
}
