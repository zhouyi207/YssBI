import { Node } from '@/shared/types/ui';
import { GraphPosition, Pin, Variable, getNodeDefinitionMeta, getLocalizedDescription } from "@/shared/types/domain";
import type {
  SerializedGraphData,
  SerializedPin,
  DeserializedNode,
  DeserializedPin,
  DeserializeGraphInput,
} from "@/shared/types/store/serialization";
import { useNodeRegistryStore } from "../nodeRegister";
import { logger } from '@/utils/appLogger';

/**
 * 将单个子图（Event, Function）序列化
 * 
 * 从节点的 Pin.links 中提取连接关系，生成独立的 connections 数组
 */
export function serializeGraph(
  id: string,
  name: string,
  type: "event" | "function",
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
    containerType: p.containerType,
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
      nodeType: node.nodeType,
      title: node.title,
      position: node.position,
      isInternal: node.isInternal,
      paramsKind: node.paramsKind,
      variableId: node.variableId,
      variableType: node.variableType,
      variableName: node.variableName,
      subGraphId: node.subGraphId,
      dataframeId: node.dataframeId,
      dataframeName: node.dataframeName,
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
    // links 必须始终从 connectionsList 重建，不能从 store 中的 pin 读取（可能是脏数据）
    return pin
      ? { ...pin, nodeId, direction, links: [] }
      : { id: String(pinIdOrObj), nodeId, name: '', type: 'any', direction, links: [] };
  };

  const nodes = (data.nodes || []).map((n) => {
    const nodeType = n.nodeType ?? '';
    const def = useNodeRegistryStore.getState().getDefinition(nodeType);
    const rawTitle = n.title ?? '';
    const useDefName = !rawTitle || rawTitle === nodeType;
    const title = def && useDefName ? def.name : (rawTitle || nodeType);

    let node: DeserializedNode;
    if (def) {
      node = {
        id: n.id,
        nodeType,
        category: n.category ?? def.category ?? [],
        title,
        position: n.position ?? { x: 0, y: 0 },
        inputs: [],
        outputs: [],
        uiStyle: n.uiStyle ?? getNodeDefinitionMeta(def)?.uiStyle ?? getNodeDefinitionMeta(def)?.ui_style ?? 'default',
        description: n.description ?? getLocalizedDescription(getNodeDefinitionMeta(def), 'en-US'),
        isInternal: !!n.isInternal,
        subGraphId: n.subGraphId,
        variableId: n.variableId,
        variableType: n.variableType,
        variableName: n.variableName,
        dataframeId: n.dataframeId,
        dataframeName: n.dataframeName,
      };
    } else {
      node = {
        id: n.id,
        nodeType,
        category: n.category ?? [],
        title,
        position: n.position ?? { x: 0, y: 0 },
        inputs: [],
        outputs: [],
        uiStyle: n.uiStyle ?? 'default',
        description: n.description,
        isInternal: !!n.isInternal,
        subGraphId: n.subGraphId,
        variableId: n.variableId,
        variableType: n.variableType,
        variableName: n.variableName,
        dataframeId: n.dataframeId,
        dataframeName: n.dataframeName,
      };
    }

    // 从 pins 中按 nodeId 和 direction 派生 inputs/outputs，以支持动态 pin（如 Decompose DataFrame）
    const nodePinsList = (data.pins || []).filter((p: { nodeId?: string }) => p.nodeId === n.id);
    node.inputs = nodePinsList
      .filter((p: { direction?: string }) => p.direction === 'input')
      .map((p: { id: string }) => resolvePin(p.id, n.id, 'input'));
    node.outputs = nodePinsList
      .filter((p: { direction?: string }) => p.direction === 'output')
      .map((p: { id: string }) => resolvePin(p.id, n.id, 'output'));

    return node;
  }).filter(Boolean) as DeserializedNode[];

  // 2. 从 connections 数组重建 Pin.links（运行时字段）
  // 使用 camelCase: { fromPin, toPin }
  for (const connection of connectionsList) {
    const sourcePin = connection.fromPin ?? connection.from;
    const targetPin = connection.toPin ?? connection.to;

    if (!sourcePin || !targetPin) {
      logger.graph.warn(`Invalid connection: ${JSON.stringify(connection)}`, 'Serialization');
      continue;
    }

    // 找到源 pin（输出 pin）
    for (const node of nodes) {
      const outputPin: DeserializedPin | undefined = node.outputs.find((p) => p.id === sourcePin);
      if (outputPin) {
        if (!outputPin.links) outputPin.links = [];
        if (!outputPin.links.includes(targetPin)) {
          outputPin.links.push(targetPin);
        }
      }

      // 找到目标 pin（输入 pin）
      const inputPin: DeserializedPin | undefined = node.inputs.find((p) => p.id === targetPin);
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
