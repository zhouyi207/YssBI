import type { GraphResourceDragData } from "@/features/core/dnd";

export function canCreateFunctionNodeInGraph(
  graphKind: "event" | "function",
  graphPath: string,
  resource: Pick<GraphResourceDragData, "type" | "id">,
): boolean {
  if (resource.type !== "function") return false;

  return (graphKind === "event" || graphKind === "function") && graphPath !== resource.id;
}
