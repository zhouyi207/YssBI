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
import type { NodeCreationDescriptor } from "@/features/domain/nodeCatalog/creationDescriptor";

/**
 * Build drag data for sidebar items (variables, functions, events, data).
 *
 * Event / function graphs use `GRAPH_RESOURCE` so the drop target can resolve the
 * current backend-issued Catalog descriptor. Event resources open their graph when
 * dropped on the canvas; function resources create a Call Function node there.
 */
export function buildSidebarDragData(
  id: string,
  name: string,
  type: "variable" | "function" | "event" | "data",
  descriptor?: NodeCreationDescriptor,
): SidebarDragPayload | null {
  if (type === "event" || type === "function") {
    return {
      type: DRAG_TYPES.GRAPH_RESOURCE,
      sidebarResource: { id, name, type },
    } satisfies GraphResourceDragPayload;
  }

  if (type === "variable" && descriptor) {
    return {
      type: DRAG_TYPES.NODE_TEMPLATE,
      template: variableNodeSpawnTemplate(descriptor, name),
    } satisfies NodeTemplateDragData;
  }
  if (type === "data" && descriptor) {
    return {
      type: DRAG_TYPES.NODE_TEMPLATE,
      template: dataFrameNodeSpawnTemplate(descriptor, name),
    } satisfies NodeTemplateDragData;
  }
  return null;
}
