import { useEditorStore } from "@/features/core/editor/stores/useEditorStore";
import type { DetailFocus } from "@/shared/types/ui/detail";

export function clearDetailFocusForClosedPanel(resourceRef: string): void {
  const focus = useEditorStore.getState().detailFocus;
  if (!focus) return;

  if (shouldClearFocus(focus, resourceRef)) {
    useEditorStore.getState().clearDetailFocus();
  }
}

function shouldClearFocus(focus: DetailFocus, resourceRef: string): boolean {
  if (focus.kind === "node" && focus.graphPath === resourceRef) return true;
  if (focus.kind === "event" || focus.kind === "function") {
    return focus.path === resourceRef;
  }
  if (focus.kind === "chart") {
    return focus.chartPath === resourceRef;
  }
  return false;
}
