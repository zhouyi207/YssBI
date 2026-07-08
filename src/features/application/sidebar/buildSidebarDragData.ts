import {
  DRAG_TYPES,
  type GraphResourceDragData,
  type GraphResourceDragPayload,
  type NodeTemplateDragData,
  type SidebarDragPayload,
} from "@/features/core/dnd";
import {
  dataFrameNodeSpawnTemplate,
  functionCallNodeSpawnTemplate,
  variableNodeSpawnTemplate,
} from "@/features/core/dnd/nodeSpawnTemplate";

/**
 * Build drag data for sidebar items (variables, functions, events, data).
 */
export function buildSidebarDragData(
  id: string,
  name: string,
  type: "variable" | "function" | "event" | "data",
): SidebarDragPayload | null {
  const sidebarResource: GraphResourceDragData | undefined =
    type === "event" || type === "function" ? { id, name, type } : undefined;

  if (type === "variable") {
    return {
      type: DRAG_TYPES.NODE_TEMPLATE,
      template: variableNodeSpawnTemplate(id, name),
    } satisfies NodeTemplateDragData;
  }
  if (type === "function") {
    return {
      type: DRAG_TYPES.NODE_TEMPLATE,
      sidebarResource,
      template: functionCallNodeSpawnTemplate(id, name),
    } satisfies NodeTemplateDragData;
  }
  if (type === "event") {
    return {
      type: DRAG_TYPES.GRAPH_RESOURCE,
      sidebarResource: sidebarResource!,
    } satisfies GraphResourceDragPayload;
  }
  if (type === "data") {
    return {
      type: DRAG_TYPES.NODE_TEMPLATE,
      template: dataFrameNodeSpawnTemplate(id, name),
    } satisfies NodeTemplateDragData;
  }
  return null;
}
