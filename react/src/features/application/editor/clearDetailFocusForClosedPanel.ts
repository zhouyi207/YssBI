import { useEditorStore } from "@/features/core/editor/stores/useEditorStore";
import { detailResourceRef } from "@/features/core/editor/detail/editorDetailPolicy";

export function clearDetailFocusForClosedPanel(
  resourceRef: string,
  panelInstanceId?: string,
): void {
  const focus = useEditorStore.getState().detailFocus;
  if (!focus) return;

  if (
    detailResourceRef(focus) === resourceRef &&
    (!panelInstanceId || !("panelInstanceId" in focus) || focus.panelInstanceId === panelInstanceId)
  ) {
    useEditorStore.getState().clearDetailFocus();
  }
}
