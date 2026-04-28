import { dataTypeDisplay } from "@/shared/types/domain/dataType";
import type { DataType } from "@/shared/types/domain";

/**
 * Build drag data for sidebar items (variables, functions, events, data).
 */
export function buildSidebarDragData(
  id: string,
  name: string,
  type: "variable" | "function" | "event" | "data",
  extra?: { dataType?: DataType | string }
) {
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
      type: "node-template",
      template: {
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
      type: "node-template",
      template: {
        nodeType: "Functions:Call Function",
        category: "Functions",
        subGraphId: id,
        subName: name,
      },
    };
  }
  if (type === "data") {
    return {
      type: "node-template",
      template: {
        nodeType: "Data:Get DataFrame",
        category: "Data",
        variableId: id,
        variableName: name,
      },
    };
  }
  return null;
}
