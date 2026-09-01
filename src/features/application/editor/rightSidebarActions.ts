import type { DetailFocus } from "@/shared/types/ui/detail";
import { useEditorStore } from "@/features/core/editor";
import { setVariablesGraphScopeFromResource } from "@/features/core/editor/detail/variablesGraphScope";
import { updateEditorGroupSelectedNodeIds } from "@/features/core/editor";
import { revealWorkbenchView } from "@/features/application/layout/workbenchLayoutActions";
import type { EditorResourceKind } from "@/features/core/dockview";

export function detailFocusForEditorResource(
  resourceKind: EditorResourceKind,
  resourceRef: string,
): DetailFocus {
  if (resourceKind === "worksheet") {
    return { kind: "worksheet", worksheetPath: resourceRef };
  }
  return { kind: resourceKind, path: resourceRef };
}

export function setDetailContext(focus: DetailFocus | null): void {
  const store = useEditorStore.getState();
  if (focus) store.setDetailFocus(focus);
  else store.clearDetailFocus();

  if (focus?.kind === "event" || focus?.kind === "function") {
    setVariablesGraphScopeFromResource(focus.path);
  }
}

/** Apply tab-derived context without replacing an explicit node inspection in the same graph. */
export function setPassiveDetailContext(focus: DetailFocus): void {
  const current = useEditorStore.getState().detailFocus;
  const preservesNodeFocus =
    (focus.kind === "event" || focus.kind === "function") &&
    current?.kind === "node" &&
    current.graphPath === focus.path;
  if (preservesNodeFocus) {
    setVariablesGraphScopeFromResource(focus.path);
    return;
  }
  setDetailContext(focus);
}

export function setInspectionContext(graphPath: string, selectedNodeIds: readonly string[]): void {
  const store = useEditorStore.getState();
  const [nodeId] = selectedNodeIds;
  if (selectedNodeIds.length === 1 && graphPath.length > 0 && nodeId?.length > 0) {
    store.setDetailFocus({ kind: "node", id: nodeId, graphPath });
  } else if (store.detailFocus?.kind === "node") {
    store.clearDetailFocus();
  }
}

export async function revealDetails(focus: DetailFocus): Promise<void> {
  setDetailContext(focus);
}

export async function revealDiagnosticNode(
  graphPath: string,
  nodeId: string,
  groupId: string,
): Promise<void> {
  updateEditorGroupSelectedNodeIds([nodeId], groupId);
  await revealDetails({ kind: "node", id: nodeId, graphPath });
}

export async function revealInspect(
  graphPath: string,
  selectedNodeIds: readonly string[],
): Promise<void> {
  setInspectionContext(graphPath, selectedNodeIds);
  const [nodeId] = selectedNodeIds;
  if (selectedNodeIds.length !== 1 || graphPath.length === 0 || !nodeId) return;
  await revealWorkbenchView("inspect");
}
