import { Node } from '@/shared/types/ui';
import { GraphPosition, Pin, Variable } from "@/shared/types/domain";
import { getNodeDefinition } from "@/features/core/nodeRegister";
import type {
  SerializedGraphData,
  SerializedPin,
  DeserializedNode,
  DeserializedPin,
  DeserializeGraphInput,
} from "@/shared/types/store/serialization";

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
  inputs: Pin[] = [],
  outputs: Pin[] = []
): SerializedGraphData {

  // 1. 提取所有连接关系
  const connections: { fromPin: string; toPin: string }[] = [];
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
              fromPin: outputPin.id,
              toPin: targetPinId
            });
            processedConnections.add(connKey);
          }
        }
      }
    }
  }

  const toSerializedPin = (p: Pin): SerializedPin => ({
    id: p.id,
    name: p.name,
    type: p.type,
    defaultValue: p.defaultValue,
    userValue: p.userValue,
    isArray: p.isArray,
  });

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
    nodes: nodes.map((node) => ({
      id: node.id,
      type: node.node_type,
      title: node.title,
      position: node.position,
      isInternal: node.isInternal,
      variableId: node.variableId,
      variableType: node.variableType,
      variableName: node.variableName,
      subGraphId: node.subGraphId,
      inputs: node.inputs.map(toSerializedPin),
      outputs: node.outputs.map(toSerializedPin),
    })),
  };
}

/**
 * 将反序列化后的节点实例还原为运行时状态
 * 
 * 从 connections 对象重建 Pin.links 字段（仅用于运行时查询）
 */
export function deserializeGraph(data: DeserializeGraphInput): {
  nodes: DeserializedNode[];
  canvas: GraphPosition;
  variables: Record<string, Variable>;
  inputs: Pin[];
  outputs: Pin[];
} {
  // Graph 类型中没有 variables，使用空对象
  const variables: Record<string, Variable> = {};

  // 处理新的 connections 格式：Connection 对象包含 connections 数组
  const connectionsData = data.connections;

  // 确保 connectionsList 是数组
  let connectionsList: Array<{ fromPin?: string; toPin?: string; from?: string; to?: string }> = [];
  if (connectionsData) {
    if (Array.isArray(connectionsData)) {
      // 如果 connections 本身就是数组（旧格式）
      connectionsList = connectionsData;
    } else if (connectionsData.connections && Array.isArray(connectionsData.connections)) {
      // 如果是新格式：{ connections: [...] }
      connectionsList = connectionsData.connections;
    } else if (typeof connectionsData === 'object') {
      connectionsList = [];
    }
  }

  // Pin ID -> 完整 Pin 对象映射（Store 格式中 nodes 只有 inputs/outputs 为 Pin ID 数组）
  const pinMap = new Map<string, SerializedPin>();
  (data.pins || []).forEach((p) => pinMap.set(p.id, p));

  const resolvePin = (pinIdOrObj: string | SerializedPin, nodeId: string, direction: 'input' | 'output'): DeserializedPin => {
    const pin = typeof pinIdOrObj === 'string' ? pinMap.get(pinIdOrObj) : pinIdOrObj;
    const pinWithLinks = pin as (SerializedPin & { links?: string[] }) | undefined;
    return pin
      ? { ...pin, nodeId, direction, links: pinWithLinks?.links ?? [] }
      : { id: String(pinIdOrObj), nodeId, name: '', type: 'any', direction, links: [] };
  };

  const nodes = (data.nodes || []).map((n) => {
    const def = getNodeDefinition(n.type ?? n.node_type ?? '');

    let node: DeserializedNode;
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

    node.inputs = (n.inputs || []).map((p) =>
      resolvePin(p, n.id, 'input')
    );
    node.outputs = (n.outputs || []).map((p) =>
      resolvePin(p, n.id, 'output')
    );

    return node;
  }).filter(Boolean) as DeserializedNode[];

  // 2. 从 connections 数组重建 Pin.links（运行时字段）
  // 使用 camelCase: { fromPin, toPin }
  for (const connection of connectionsList) {
    const sourcePin = connection.fromPin ?? connection.from;
    const targetPin = connection.toPin ?? connection.to;

    if (!sourcePin || !targetPin) {
      console.warn('[deserializeGraph] Invalid connection:', connection);
      continue;
    }

    // 找到源 pin（输出 pin）
    for (const node of nodes) {
      const outputPin = node.outputs.find((p) => p.id === sourcePin);
      if (outputPin) {
        if (!outputPin.links) outputPin.links = [];
        if (!outputPin.links.includes(targetPin)) {
          outputPin.links.push(targetPin);
        }
      }

      // 找到目标 pin（输入 pin）
      const inputPin = node.inputs.find((p) => p.id === targetPin);
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
