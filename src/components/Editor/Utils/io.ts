import { BaseNode } from "../Types/nodes";
import { CanvasState, SubGraphData } from "../Types/canvas";
import { getNodeDefinition } from "../Hooks/useNodeRegistry";
import { VariableDefinition } from "../Types/variables";

/**
 * 将单个子图（Event, Function, Macro）序列化
 */
export function serializeSubGraph(
  id: string,
  name: string,
  type: "event" | "function" | "macro",
  nodes: BaseNode[],
  canvas: CanvasState,
  variables: Record<string, VariableDefinition>,
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
      variableType: node.variableType,
      variableName: node.variableName,
      subGraphId: node.subGraphId,
      inputs: node.inputs.map((p) => ({
        id: p.id,
        name: p.name,
        type: p.type,
        links: p.links,
        defaultValue: p.defaultValue,
        isArray: p.isArray,
      })),
      outputs: node.outputs.map((p) => ({
        id: p.id,
        name: p.name,
        type: p.type,
        links: p.links,
        defaultValue: p.defaultValue,
        isArray: p.isArray,
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
  variables: Record<string, VariableDefinition>;
  inputs: import("../Types/canvas").PinDefinition[];
  outputs: import("../Types/canvas").PinDefinition[];
} {
  const variables: Record<string, VariableDefinition> = data.variables || {};


  const nodes = (data.nodes || []).map((n: any) => {
    const def = getNodeDefinition(n.type);

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

    // 变量节点相关字段
    node.variableId = n.variableId;
    node.variableType = n.variableType;
    node.variableName = n.variableName;

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