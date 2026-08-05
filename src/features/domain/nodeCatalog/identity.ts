export type NodeTypeId = string;

export const BUILTIN_NODE_TYPE_IDS = {
  callFunction: 'yssbi.project.function.call',
  getVariable: 'yssbi.project.variable.get',
  setVariable: 'yssbi.project.variable.set',
  getDataframe: 'yssbi.dataframe.source.get',
} as const satisfies Record<string, NodeTypeId>;

export type VariableNodeTypeId =
  | typeof BUILTIN_NODE_TYPE_IDS.getVariable
  | typeof BUILTIN_NODE_TYPE_IDS.setVariable;

export function isCallFunctionNodeType(value: string | undefined): boolean {
  return value === BUILTIN_NODE_TYPE_IDS.callFunction;
}

export function isVariableNodeType(value: string | undefined): boolean {
  return value === BUILTIN_NODE_TYPE_IDS.getVariable
    || value === BUILTIN_NODE_TYPE_IDS.setVariable;
}

export function isDatabaseResourceNodeType(value: string | undefined): boolean {
  return value === BUILTIN_NODE_TYPE_IDS.getDataframe;
}
