import { shallow } from "zustand/shallow";
import type { DetailFocus } from "@/features/core/editor/detail/detailTypes";
import { useEditorStore } from "@/features/core/editor";
import { useResourceStore } from "@/features/core/resource";
import { revealWorkbenchView, workbenchLayoutRead } from "@/modules/workbench/public";
import type { EditorResourceKind } from "@/modules/workbench/public";

export function detailFocusForEditorResource(
  resourceKind: EditorResourceKind,
  resourceRef: string,
  panelInstanceId: string,
): DetailFocus | null {
  if (resourceKind === "doc") return null;
  if (resourceKind === "mind") return { kind: "mind", path: resourceRef, panelInstanceId };
  if (resourceKind === "chart") {
    return { kind: "chart", chartPath: resourceRef };
  }
  if (resourceKind === "database") return { kind: "data", id: resourceRef };
  return { kind: resourceKind, path: resourceRef };
}

export function setDetailContext(focus: DetailFocus | null): void {
  const store = useEditorStore.getState();
  if (focus) store.setDetailFocus(focus);
  else store.clearDetailFocus();
}

/** Apply tab-derived context without replacing an explicit node inspection in the same graph. */
export function setPassiveDetailContext(focus: DetailFocus | null): void {
  const current = useEditorStore.getState().detailFocus;
  const preservesNodeFocus =
    (focus?.kind === "event_graph" || focus?.kind === "function_graph") &&
    current?.kind === "node" &&
    current.graphPath === focus.path;
  if (preservesNodeFocus || shallow(current, focus)) {
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
    const resource = Object.values(useResourceStore.getState().resources).find(
      (resource) =>
        resource.id === graphPath &&
        (resource.kind === "event_graph" || resource.kind === "function_graph"),
    );
    if (resource?.kind === "event_graph" || resource?.kind === "function_graph")
      store.setDetailFocus({ kind: resource.kind, path: graphPath });
    else store.clearDetailFocus();
  }
}

export async function revealDetails(focus: DetailFocus): Promise<void> {
  setDetailContext(focus);
  if (workbenchLayoutRead.isReady) await revealWorkbenchView("details");
}
