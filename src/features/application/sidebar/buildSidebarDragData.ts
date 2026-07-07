import type { DataType } from "@/shared/types/domain";
import { DRAG_TYPES } from "@/features/core/dnd";
import { CALL_FUNCTION_NODE_TYPE } from "@/features/domain/nodeDefinition";

/**
 * Build drag data for sidebar items (variables, functions, events, data).
 */
export function buildSidebarDragData(
  id: string,
  name: string,
  type: "variable" | "function" | "event" | "data",
  extra?: { dataType?: DataType | string },
) {
  void extra;
  const sidebarResource = type === "event" || type === "function"
    ? { id, name, type }
    : undefined;

  if (type === "variable") {
    return {
      type: DRAG_TYPES.NODE_TEMPLATE,
      template: {
        title: name,
        nodeType: "Variables:Get Variable",
        category: "Variable",
        variableId: id,
        variableName: name,
      },
    };
  }
  if (type === "function") {
    return {
      type: DRAG_TYPES.NODE_TEMPLATE,
      sidebarResource,
      template: {
        title: name,
        nodeType: CALL_FUNCTION_NODE_TYPE,
        category: "Functions",
        subGraphId: id,
      },
    };
  }
  if (type === "event") {
    return {
      type: DRAG_TYPES.GRAPH_RESOURCE,
      sidebarResource,
    };
  }
  if (type === "data") {
    return {
      type: DRAG_TYPES.NODE_TEMPLATE,
      template: {
        title: name,
        nodeType: "Data:Get DataFrame",
        category: "Data",
        variableId: id,
        variableName: name,
      },
    };
  }
  return null;
}
