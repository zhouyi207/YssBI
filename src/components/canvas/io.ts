import { BaseNode } from "../node/models";
import { CanvasState } from "./type";
import { NODE_REGISTRY } from "../node/registry";

/**
 * 节点编辑器数据协议格式 (Schema)
 */
export interface GraphData {
  version: string;
  canvas: CanvasState;
  nodes: {
    id: string;
    type: string;
    title: string;
    position: { x: number; y: number };
    selected?: boolean;
    variableId?: string;
    inputs: any[];
    outputs: any[];
  }[];
  variables?: Record<string, { name: string; type: string; value: any; scope?: "local" | "global" }>;
  functions?: Record<string, { name: string }>;
  macros?: Record<string, { name: string }>;
  metadata: {
    exportTime: string;
    appVersion: string;
  };
}

const CURRENT_VERSION = "1.0.0";

/**
 * 将当前画布状态序列化为可存储的 JSON 对象
 */
export function serializeGraph(
  nodes: BaseNode[],
  canvas: CanvasState,
  localVariables?: Record<string, { name: string; type: string; value: any }>,
  globalVariables?: Record<string, { name: string; type: string; value: any }>,
  functions?: Record<string, { name: string }>,
  macros?: Record<string, { name: string }>
): GraphData {
  const mergedVariables: Record<string, { name: string; type: string; value: any; scope?: "local" | "global" }> = {};
  
  if (localVariables) {
    Object.entries(localVariables).forEach(([id, data]) => {
      mergedVariables[id] = { ...data, scope: "local" };
    });
  }
  
  if (globalVariables) {
    Object.entries(globalVariables).forEach(([id, data]) => {
      mergedVariables[id] = { ...data, scope: "global" };
    });
  }

  return {
    version: CURRENT_VERSION,
    canvas,
    nodes: nodes.map((node) => ({
      id: node.id,
      type: node.type,
      title: node.title,
      position: node.position,
      selected: node.selected,
      variableId: node.variableId,
      inputs: node.inputs.map((p) => ({
        id: p.id,
        name: p.name,
        type: p.type,
        links: p.links,
        defaultValue: p.defaultValue,
      })),
      outputs: node.outputs.map((p) => ({
        id: p.id,
        name: p.name,
        type: p.type,
        links: p.links,
        defaultValue: p.defaultValue,
      })),
    })),
    variables: mergedVariables,
    functions,
    macros,
    metadata: {
      exportTime: new Date().toISOString(),
      appVersion: "0.1.0",
    },
  };
}

/**
 * 将 JSON 对象反序列化为运行时的节点实例
 */
export function deserializeGraph(data: any): { 
  nodes: BaseNode[]; 
  canvas: CanvasState; 
  localVariables: Record<string, { name: string; type: string; value: any }>;
  globalVariables: Record<string, { name: string; type: string; value: any }>;
  functions: Record<string, { name: string }>;
  macros: Record<string, { name: string }>;
} {
  if (!data || data.version !== CURRENT_VERSION) {
    console.warn("Graph data version mismatch or invalid data");
  }

  const variables = data.variables || {};
  const localVariables: Record<string, any> = {};
  const globalVariables: Record<string, any> = {};

  Object.entries(variables).forEach(([id, data]: [string, any]) => {
    const { scope, ...rest } = data;
    if (scope === "global") {
      globalVariables[id] = rest;
    } else {
      localVariables[id] = rest;
    }
  });

  const nodes = (data.nodes || []).map((n: any) => {
    const def = NODE_REGISTRY.getDefinition(n.type);
    if (!def) {
      console.error(`Unknown node type: ${n.type}`);
      return null;
    }

    if (n.variableId && !variables[n.variableId]) {
      console.warn(`Node ${n.id} refers to missing variable ${n.variableId}. Skipping node.`);
      return null;
    }

    const node = new BaseNode(n.id, def, n.position);
    node.title = n.title;
    node.selected = !!n.selected;
    node.variableId = n.variableId;

    node.inputs = (n.inputs || []).map((p: any) => ({
      ...p,
      nodeId: n.id,
      direction: "input",
    }));

    node.outputs = (n.outputs || []).map((p: any) => ({
      ...p,
      nodeId: n.id,
      direction: "output",
    }));

    return node;
  }).filter(Boolean) as BaseNode[];

  return {
    nodes,
    canvas: data.canvas || { x: 0, y: 0, scale: 1 },
    localVariables,
    globalVariables,
    functions: data.functions || {},
    macros: data.macros || {},
  };
}
