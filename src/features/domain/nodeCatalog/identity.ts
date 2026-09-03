export type NodeTypeId = string;

export const BUILTIN_NODE_TYPE_IDS = {
  callFunction: "yssbi.project.function.call",
  getVariable: "yssbi.project.variable.get",
  getDataframe: "yssbi.dataframe.source.get",
} as const satisfies Record<string, NodeTypeId>;

export type VariableNodeTypeId = typeof BUILTIN_NODE_TYPE_IDS.getVariable;

export function isCallFunctionNodeType(value: string | undefined): boolean {
  return value === BUILTIN_NODE_TYPE_IDS.callFunction;
}

export function isVariableNodeType(value: string | undefined): boolean {
  return value === BUILTIN_NODE_TYPE_IDS.getVariable;
}

export function isDatabaseResourceNodeType(value: string | undefined): boolean {
  return value === BUILTIN_NODE_TYPE_IDS.getDataframe;
}
