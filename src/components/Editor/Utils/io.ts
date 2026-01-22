import { BaseNode } from "../Types/nodes";
import { CanvasState, ProjectData, SubGraphData } from "../Types/canvas";
import { NODE_REGISTRY } from "../Nodes/registry";


const CURRENT_VERSION = "1.0.0";

/**
 * 将单个子图（Event, Function, Macro）序列化
 */
export function serializeSubGraph(
  id: string,
  name: string,
  type: "event" | "function" | "macro",
  nodes: BaseNode[],
  canvas: CanvasState,
  variables: Record<string, { name: string; type: string; value: any }>,
  inputs: import("../Types/canvas").PinDefinition[] = [],
  outputs: import("../Types/canvas").PinDefinition[] = []
): SubGraphData {

  return {
    id,
    name,
    type,
    canvas,
    variables,
    inputs,
    outputs,
    nodes: nodes.map((node) => ({
      id: node.id,
      type: node.type,
      title: node.title,
      position: node.position,

      isInternal: node.isInternal,
      variableId: node.variableId,
      subGraphId: node.subGraphId,
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
  };
}

/**
 * 将反序列化后的节点实例还原为运行时状态
 */
export function deserializeSubGraph(data: SubGraphData): {
  nodes: BaseNode[];
  canvas: CanvasState;
  variables: Record<string, { name: string; type: string; value: any }>;
  inputs: import("../Types/canvas").PinDefinition[];
  outputs: import("../Types/canvas").PinDefinition[];
} {


  const variables = data.variables || {};


  const nodes = (data.nodes || []).map((n: any) => {
    const def = NODE_REGISTRY.getDefinition(n.type);

    let node: BaseNode;
    if (def) {
      node = new BaseNode(n.id, def, n.position);
    } else {
      // Create a shell node if definition is missing (common for internal entry/return nodes)
      node = new BaseNode(n.id, {
        node_type: n.type,
        category: "Internal",
        title: n.title,
        inputs: [],
        outputs: [],
        ui_style: "default"
      }, n.position);
    }

    node.isInternal = !!n.isInternal;
    node.subGraphId = n.subGraphId;
    node.title = n.title;

    node.variableId = n.variableId;

    if (n.variableId && !variables[n.variableId]) {
      console.warn(`Node ${n.id} in ${data.name} refers to missing variable ${n.variableId}.`);
    }

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
    variables,
    inputs: data.inputs || [],
    outputs: data.outputs || [],
  };
}
/**
 * 序列化整个项目
 */
export function serializeProject(
  globalVariables: Record<string, { name: string; type: string; value: any }>,
  events: Record<string, SubGraphData>,
  functions: Record<string, SubGraphData>,
  macros: Record<string, SubGraphData>
): ProjectData {
  return {
    version: CURRENT_VERSION,
    globalVariables,
    events,
    functions,
    macros,
    metadata: {
      exportTime: new Date().toISOString(),
      appVersion: "0.1.0",
    },
  };
}

/**
 * 反序列化整个项目
 */
export function deserializeProject(jsonStr: string): ProjectData {
  try {
    const data = JSON.parse(jsonStr);
    if (data.version !== CURRENT_VERSION) {
      console.warn("Project version mismatch");
    }
    return {
      version: data.version || CURRENT_VERSION,
      globalVariables: data.globalVariables || {},
      events: data.events || {},
      functions: data.functions || {},
      macros: data.macros || {},
      metadata: data.metadata || { exportTime: new Date().toISOString(), appVersion: "0.1.0" },
    };
  } catch (e) {
    console.error("Failed to parse project JSON", e);
    throw e;
  }
}