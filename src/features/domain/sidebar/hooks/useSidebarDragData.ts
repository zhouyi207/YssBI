import { dataTypeDisplay } from "@/shared/types/domain/dataType";

/**
 * Build drag data for sidebar items (variables, functions, macros, events, data).
 * Extracted from Sidebar.tsx - view should only consume this hook.
 */
export function buildSidebarDragData(
  id: string,
  name: string,
  type: "variable" | "function" | "macro" | "event" | "data",
  extra?: { dataType?: import("@/shared/types/domain").DataType | string }
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
        nodeType: "get_variable",
        category: "Variable",
        variableId: id,
        variableName: name,
        variableType,
        containerType,
      },
    };
  }
  if (type === "function" || type === "macro") {
    return {
      type: "node-template",
      template: {
        nodeType: `call_${type}`,
        category: type === "function" ? "Functions" : "Macros",
        subGraphId: id,
        subName: name,
      },
    };
  }
  if (type === "data") {
    return {
      type: "node-template",
      template: {
        nodeType: "get_dataframe",
        category: "Data",
        variableId: id,
        variableName: name,
      },
    };
  }
  return null;
}

export function buildColumnDragData(
  dataframeId: string,
  _columnIndex: number,
  col: { name: string; type: string }
) {
  return {
    type: "node-template",
    template: {
      nodeType: "get_column",
      category: "Data",
      title: `Get ${col.name}`,
      initialData: {
        columnName: col.name,
        columnType: col.type,
        dataframeId,
      },
    },
  };
}
