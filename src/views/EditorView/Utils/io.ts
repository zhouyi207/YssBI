import { BaseNode } from "../Types/nodes";
import { CanvasState, SubGraphData, Connection } from "../Types/canvas";
import { getNodeDefinition } from "@/features/node-registry";
import { VariableDefinition } from "../Types/variables";

/**
 * 将单个子图（Event, Function, Macro）序列化
 * 
 * 从节点的 Pin.links 中提取连接关系，生成独立的 connections 数组
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

  // 1. 提取所有连接关系
  const connections: Connection[] = [];
  const processedConnections = new Set<string>(); // 防止重复添加

  for (const node of nodes) {
    // 处理输出 pin 的连接（输出 pin 是连接的源）
    for (const outputPin of node.outputs) {
      if (outputPin.links && outputPin.links.length > 0) {
        for (const targetPinId of outputPin.links) {
          // 创建连接 ID（使用排序后的 pin ID 确保唯一性）
          const connKey = `${outputPin.id}->${targetPinId}`;
          
          if (!processedConnections.has(connKey)) {
            connections.push({
              id: `conn-${crypto.randomUUID()}`,
              sourcePin: outputPin.id,
              targetPin: targetPinId
            });
            processedConnections.add(connKey);
          }
        }
      }
    }
  }

  // 2. 序列化节点（不包含 links 字段）
  return {
    id,
    name,
    type,
    canvas,
    variables,
    inputs,
    outputs,
    connections,  // 独立的连接数组
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
        // links 字段不再序列化
        defaultValue: p.defaultValue,
        userValue: p.userValue,
        isArray: p.isArray,
      })),
      outputs: node.outputs.map((p) => ({
        id: p.id,
        name: p.name,
        type: p.type,
        // links 字段不再序列化
        defaultValue: p.defaultValue,
        userValue: p.userValue,
        isArray: p.isArray,
      })),
    })),
  };
}

/**
 * 将反序列化后的节点实例还原为运行时状态
 * 
 * 从 connections 数组重建 Pin.links 字段（仅用于运行时查询）
 */
export function deserializeSubGraph(data: SubGraphData): {
  nodes: BaseNode[];
  canvas: CanvasState;
  variables: Record<string, VariableDefinition>;
  inputs: import("../Types/canvas").PinDefinition[];
  outputs: import("../Types/canvas").PinDefinition[];
} {
  const variables: Record<string, VariableDefinition> = data.variables || {};
  const connections: Connection[] = data.connections || [];

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
      links: [],  // 初始化为空，稍后从 connections 填充
    }));

    node.outputs = (n.outputs || []).map((p: any) => ({
      ...p,
      nodeId: n.id,
      direction: "output",
      links: [],  // 初始化为空，稍后从 connections 填充
    }));

    return node;
  }).filter(Boolean) as BaseNode[];

  // 2. 从 connections 数组重建 Pin.links（运行时字段）
  for (const connection of connections) {
    // 找到源 pin（输出 pin）
    for (const node of nodes) {
      const outputPin = node.outputs.find(p => p.id === connection.sourcePin);
      if (outputPin) {
        if (!outputPin.links) outputPin.links = [];
        if (!outputPin.links.includes(connection.targetPin)) {
          outputPin.links.push(connection.targetPin);
        }
      }

      // 找到目标 pin（输入 pin）
      const inputPin = node.inputs.find(p => p.id === connection.targetPin);
      if (inputPin) {
        if (!inputPin.links) inputPin.links = [];
        if (!inputPin.links.includes(connection.sourcePin)) {
          inputPin.links.push(connection.sourcePin);
        }
      }
    }
  }

  return {
    nodes,
    canvas: data.canvas || { x: 0, y: 0, scale: 1 },
    variables,
    inputs: data.inputs || [],
    outputs: data.outputs || [],
  };
}