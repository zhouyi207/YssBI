import { shallow } from "zustand/shallow";
import type { DetailFocus } from "@/features/core/editor/detail/detailTypes";
import {
  resolveEditorDetailFocus,
  type EditorDetailScope,
} from "@/features/core/editor/detail/editorDetailPolicy";
import { useEditorStore } from "@/features/core/editor";
import {
  getPaneSelection,
  revealWorkbenchView,
  workbenchLayoutRead,
} from "@/modules/workbench/public";
import type { EditorResourceKind } from "@/modules/workbench/public";

export function detailFocusForEditorResource(
  resourceKind: EditorResourceKind,
  resourceRef: string,
  panelInstanceId: string,
): DetailFocus {
  return resolveEditorDetailFocus(
    { resourceKind, resourceRef, panelInstanceId },
    getPaneSelection(panelInstanceId).selectedNodeIds,
  );
}

export function setDetailContext(focus: DetailFocus | null): void {
  const store = useEditorStore.getState();
  if (shallow(store.detailFocus, focus)) return;
  if (focus) store.setDetailFocus(focus);
  else store.clearDetailFocus();
}

export function setInspectionContext(
  scope: EditorDetailScope,
  selectedNodeIds: readonly string[],
): void {
  const active = workbenchLayoutRead.getActiveEditorPanel();
  if (
    active?.panelInstanceId !== scope.panelInstanceId ||
    active.metadata.resourceRef !== scope.resourceRef ||
    active.metadata.resourceKind !== scope.resourceKind
  )
    return;
  setDetailContext(resolveEditorDetailFocus(scope, selectedNodeIds));
}

export async function revealDetails(focus: DetailFocus): Promise<void> {
  setDetailContext(focus);
  if (workbenchLayoutRead.isReady) await revealWorkbenchView("details");
}
