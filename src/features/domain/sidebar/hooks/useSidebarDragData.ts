/**
 * Build drag data for sidebar items (variables, functions, macros, events, data).
 * Extracted from Sidebar.tsx - view should only consume this hook.
 */
export function buildSidebarDragData(
  id: string,
  name: string,
  type: "variable" | "function" | "macro" | "event" | "data",
  extra?: { data_type?: string; is_array?: boolean }
) {
  if (type === "variable") {
    return {
      type: "node-template",
      template: {
        type: "get_variable",
        category: "Variable",
        variableId: id,
        variableName: name,
        variableType: extra?.data_type,
        variableIsArray: extra?.is_array,
      },
    };
  }
  if (type === "function" || type === "macro") {
    return {
      type: "node-template",
      template: {
        type: `call_${type}`,
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
        type: "get_dataframe",
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
      type: "get_column",
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
