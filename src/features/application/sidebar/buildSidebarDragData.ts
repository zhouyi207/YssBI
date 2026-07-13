import {
  DRAG_TYPES,
  type GraphResourceDragPayload,
  type NodeTemplateDragData,
  type SidebarDragPayload,
} from "@/features/core/dnd";
import {
  dataFrameNodeSpawnTemplate,
  variableNodeSpawnTemplate,
} from "@/features/core/dnd/nodeSpawnTemplate";

/**
 * Build drag data for sidebar items (variables, functions, events, data).
 *
 * Event / function graphs use `GRAPH_RESOURCE` — drop on editor canvas or TabBar opens
 * the graph tab (canvas: target group; TabBar: pinned at insert index). Call Function nodes
 * are spawned from the in-graph Node Palette (`buildContextualCatalogItems`), not from graphs sidebar.
 */
export function buildSidebarDragData(
  id: string,
  name: string,
  type: "variable" | "function" | "event" | "data",
): SidebarDragPayload | null {
  if (type === "event" || type === "function") {
    return {
      type: DRAG_TYPES.GRAPH_RESOURCE,
      sidebarResource: { id, name, type },
    } satisfies GraphResourceDragPayload;
  }

  if (type === "variable") {
    return {
      type: DRAG_TYPES.NODE_TEMPLATE,
      template: variableNodeSpawnTemplate(id, name),
    } satisfies NodeTemplateDragData;
  }
  if (type === "data") {
    return {
      type: DRAG_TYPES.NODE_TEMPLATE,
      template: dataFrameNodeSpawnTemplate(id, name),
    } satisfies NodeTemplateDragData;
  }
  return null;
}
