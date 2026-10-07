import type { ResourceKind } from "@/shared/types/domain/resource";
import type { ResourceRef } from "@/features/domain/resource/resourceTypes";
import type { DetailFocus } from "./detailTypes";

export const EDITOR_DETAIL_POLICIES = {
  event_graph: "selection",
  function_graph: "selection",
  mind: "selection",
  doc: "resource",
  chart: "resource",
  database: "resource",
} as const satisfies Record<ResourceKind, "selection" | "resource">;

export interface EditorDetailScope {
  resourceKind: ResourceKind;
  resourceRef: string;
  panelInstanceId: string;
}

/** The same rule resolves tab activation, selection changes and file-level fallback. */
export function resolveEditorDetailFocus(
  scope: EditorDetailScope,
  selectedNodeIds: readonly string[],
): DetailFocus {
  const { resourceKind, resourceRef, panelInstanceId } = scope;
  const nodeId =
    EDITOR_DETAIL_POLICIES[resourceKind] === "selection" && selectedNodeIds.length === 1
      ? selectedNodeIds[0]
      : null;
  if (resourceKind === "mind") return { kind: "mind", path: resourceRef, panelInstanceId, nodeId };
  if (nodeId) return { kind: "node", id: nodeId, graphPath: resourceRef };
  if (resourceKind === "chart") return { kind: "chart", chartPath: resourceRef };
  if (resourceKind === "database") return { kind: "data", id: resourceRef };
  return { kind: resourceKind, path: resourceRef };
}

export function detailResource(
  focus: DetailFocus | null,
  resources: Readonly<Record<string, ResourceRef>> = {},
): ResourceRef | null {
  if (!focus) return null;
  if (focus.kind === "node") {
    const resource = Object.values(resources).find(
      (resource) =>
        resource.id === focus.graphPath &&
        (resource.kind === "event_graph" || resource.kind === "function_graph"),
    );
    return resource ? { kind: resource.kind, id: resource.id } : null;
  }
  if (focus.kind === "chart") return { kind: "chart", id: focus.chartPath };
  if (focus.kind === "data") return { kind: "database", id: focus.id };
  if ("path" in focus) return { kind: focus.kind, id: focus.path };
  return null;
}

export function detailResourceRef(focus: DetailFocus | null): string | null {
  return focus?.kind === "node" ? focus.graphPath : (detailResource(focus)?.id ?? null);
}

export function remapDetailResource(
  focus: DetailFocus | null,
  from: string,
  to: string,
): DetailFocus | null {
  if (!focus || from === to || detailResourceRef(focus) !== from) return focus;
  if (focus.kind === "node") return { ...focus, graphPath: to };
  if (focus.kind === "chart") return { ...focus, chartPath: to };
  if (focus.kind === "data") return { ...focus, id: to };
  if ("path" in focus) return { ...focus, path: to };
  return focus;
}
