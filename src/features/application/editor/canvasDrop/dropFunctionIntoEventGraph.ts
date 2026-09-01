import type { GraphResourceDragData } from "@/features/core/dnd";
import { EDITOR_MUTATION_CAPABILITIES } from "../editorMutationAvailability";

export function canCreateFunctionNodeInGraph(
  graphKind: "event" | "function",
  graphPath: string,
  resource: Pick<GraphResourceDragData, "type" | "id">,
): boolean {
  if (!EDITOR_MUTATION_CAPABILITIES.resourceBoundDescriptors) return false;
  if (resource.type !== "function") return false;

  return (graphKind === "event" || graphKind === "function") && graphPath !== resource.id;
}
