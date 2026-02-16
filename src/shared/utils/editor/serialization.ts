import { Node } from '@/shared/types/ui';
import { GraphPosition } from "@/shared/types/domain";
import { getNodeDefinition } from "@/features/core/nodeRegister";
import { Variable } from "@/shared/types/domain";

/**
 * 将单个子图（Event, Function, Macro）序列化
 * 
 * 从节点的 Pin.links 中提取连接关系，生成独立的 connections 数组
 */
export function serializeGraph(
  id: string,
  name: string,
  type: "event" | "function" | "macro",
  nodes: Node[],
  canvas: GraphPosition,
  variables: Record<string, Variable>,
  inputs: import("@/shared/types/domain").Pin[] = [],
  outputs: import("@/shared/types/domain").Pin[] = []
): any {

  // 1. 提取所有连接关系
  const connections: { from_pin: string; to_pin: string }[] = [];
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
              from_pin: outputPin.id,
              to_pin: targetPinId
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
    connections: { connections },  // 包装为 ConnectionDTO 格式
    nodes: nodes.map((node: any) => ({
      id: node.id,
      type: node.type,
      title: node.title,
      position: node.position,

      isInternal: node.isInternal,
      variableId: node.variableId,
      variableType: node.variableType,
      variableName: node.variableName,
      subGraphId: node.subGraphId,
      inputs: node.inputs.map((p: any) => ({
        id: p.id,
        name: p.name,
        type: p.type,
        // links 字段不再序列化
        defaultValue: p.defaultValue,
        userValue: p.userValue,
        isArray: p.isArray,
      })),
      outputs: node.outputs.map((p: any) => ({
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
 * 从 connections 对象重建 Pin.links 字段（仅用于运行时查询）
 */
export function deserializeGraph(data: any): {
  nodes: any[];
  canvas: GraphPosition;
  variables: Record<string, Variable>;
  inputs: any[];
  outputs: any[];
} {


  // Graph 类型中没有 variables，使用空对象
  const variables: Record<string, Variable> = {};

  // 处理新的 connections 格式：Connection 对象包含 connections 数组
  const connectionsData = data.connections as any;

  // 确保 connectionsList 是数组
  let connectionsList: any[] = [];
  if (connectionsData) {
    if (Array.isArray(connectionsData)) {
      // 如果 connections 本身就是数组（旧格式）
      connectionsList = connectionsData;
    } else if (connectionsData.connections && Array.isArray(connectionsData.connections)) {
      // 如果是新格式：{ connections: [...] }
      connectionsList = connectionsData.connections;
    } else if (typeof connectionsData === 'object') {
      // 如果是对象但没有 connections 数组，转换为空数组
      // console.warn('[deserializeGraph] connections is an object but not in expected format:', connectionsData);
      connectionsList = [];
    }
  }

  // Pin ID -> 完整 Pin 对象映射（Store 格式中 nodes 只有 inputs/outputs 为 Pin ID 数组）
  const pinMap = new Map<string, any>();
  (data.pins || []).forEach((p: any) => pinMap.set(p.id, p));

  const resolvePin = (pinIdOrObj: any, nodeId: string, direction: 'input' | 'output') => {
    const pin = typeof pinIdOrObj === 'string' ? pinMap.get(pinIdOrObj) : pinIdOrObj;
    return pin
      ? { ...pin, nodeId, direction, links: pin.links ?? [] }
      : { id: pinIdOrObj, nodeId, direction, links: [], name: '', type: 'any' };
  };

  const nodes = (data.nodes || []).map((n: any) => {
    const def = getNodeDefinition(n.type ?? n.node_type);

    let node: any;
    if (def) {
      node = {
        id: n.id,
        type: n.type ?? n.node_type,
        node_type: n.node_type ?? n.type,
        category: n.category ?? def.category ?? [],
        title: n.title ?? def.name,
        position: n.position,
        inputs: [],
        outputs: [],
        ui_style: n.ui_style ?? def.node_metadata?.ui_style ?? 'default',
        description: n.description ?? def.node_metadata?.description,
        isInternal: !!n.isInternal,
        subGraphId: n.subGraphId,
        variableId: n.variableId,
        variableType: n.variableType,
        variableName: n.variableName,
      };
    } else {
      node = {
        id: n.id,
        type: n.type ?? n.node_type,
        node_type: n.node_type ?? n.type,
        category: n.category ?? [],
        title: n.title ?? n.type,
        position: n.position,
        inputs: [],
        outputs: [],
        ui_style: n.ui_style ?? 'default',
        description: n.description,
        isInternal: !!n.isInternal,
        subGraphId: n.subGraphId,
        variableId: n.variableId,
        variableType: n.variableType,
        variableName: n.variableName,
      };
    }

    node.inputs = (n.inputs || []).map((p: any) =>
      resolvePin(p, n.id, 'input')
    );
    node.outputs = (n.outputs || []).map((p: any) =>
      resolvePin(p, n.id, 'output')
    );

    return node;
  }).filter(Boolean);

  // 2. 从 connections 数组重建 Pin.links（运行时字段）
  // 支持 { from_pin, to_pin } 或 { from, to }
  for (const connection of connectionsList) {
    const sourcePin = connection.from_pin ?? connection.from;
    const targetPin = connection.to_pin ?? connection.to;

    if (!sourcePin || !targetPin) {
      console.warn('[deserializeGraph] Invalid connection:', connection);
      continue;
    }

    // 找到源 pin（输出 pin）
    for (const node of nodes) {
      const outputPin = node.outputs.find((p: any) => p.id === sourcePin);
      if (outputPin) {
        if (!outputPin.links) outputPin.links = [];
        if (!outputPin.links.includes(targetPin)) {
          outputPin.links.push(targetPin);
        }
      }

      // 找到目标 pin（输入 pin）
      const inputPin = node.inputs.find((p: any) => p.id === targetPin);
      if (inputPin) {
        if (!inputPin.links) inputPin.links = [];
        if (!inputPin.links.includes(sourcePin)) {
          inputPin.links.push(sourcePin);
        }
      }
    }
  }



  return {
    nodes,
    canvas: data.canvas || { x: 0, y: 0, scale: 1 },
    variables,
    inputs: [],  // Graph 类型中没有 inputs
    outputs: [], // Graph 类型中没有 outputs
  };
}
