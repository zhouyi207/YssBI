import { dataTypeDisplay } from "@/shared/types/domain/dataType";
import type { DataType } from "@/shared/types/domain";
import { DRAG_TYPES } from "@/features/core/dnd";

/**
 * Build drag data for sidebar items (variables, functions, events, data).
 */
export function buildSidebarDragData(
  id: string,
  name: string,
  type: "variable" | "function" | "event" | "data",
  extra?: { dataType?: DataType | string; folderPath?: string }
) {
  const sidebarResource = type === "event" || type === "function"
    ? { id, name, type, folderPath: extra?.folderPath ?? "" }
    : undefined;

  if (type === "variable") {
    const dt = extra?.dataType;
    const variableType = dt
      ? typeof dt === "string"
        ? dt
        : dataTypeDisplay(dt)
      : undefined;
    const containerType = dt && typeof dt === "object" && "kind" in dt
      ? dt.kind === "Array" ? "array" : dt.kind === "DataSeries" ? "dataseries" : undefined
      : undefined;
    return {
      type: DRAG_TYPES.NODE_TEMPLATE,
      template: {
        title: name,
        nodeType: "Variables:Get Variable",
        category: "Variable",
        variableId: id,
        variableName: name,
        variableType,
        containerType,
      },
    };
  }
  if (type === "function") {
    return {
      type: DRAG_TYPES.NODE_TEMPLATE,
      sidebarResource,
      template: {
        title: name,
        nodeType: "Functions:Call Function",
        category: "Functions",
        subGraphId: id,
        subName: name,
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
