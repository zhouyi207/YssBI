import type { GraphResourceDragData } from "@/features/core/dnd";

export function canCreateFunctionNodeInGraph(
  graphKind: "event_graph" | "function_graph",
  graphPath: string,
  resource: Pick<GraphResourceDragData, "type" | "id">,
): boolean {
  if (resource.type !== "function_graph") return false;

  return (
    (graphKind === "event_graph" || graphKind === "function_graph") && graphPath !== resource.id
  );
}
