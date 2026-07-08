/**
 * GraphData（store）↔ domain Graph 显式转换
 * 项目快照导出 / hydrate 边界单点，避免 `as unknown as` 漂移。
 */

import type { Connection, ConnectionItem } from '../domain/connection';
import type { Graph } from '../domain/graph';
import type { Pin, PinType } from '../domain/pin';
import type { GraphInstanceDTO } from './graph';
import {
  connectionDataToItems,
  connectionItemToConnectionData,
} from './graphConverters';
import type {
  ConnectionData,
  GraphData,
  GraphDataLike,
  NodeData,
  PinData,
  RuntimeNodeInput,
} from '../store/graph';

type DomainGraphNode = Graph['nodes'][number] & {
  position?: { x: number; y: number };
  paramsKind?: NodeData['paramsKind'];
  variableId?: string;
  variableName?: string;
  variableType?: string;
  subGraphId?: string;
  dataframeId?: string;
  isInternal?: boolean;
};

/** 兼容 hydrate 入站（DTO / domain / 历史快照）的多种 connections 形态 → store `ConnectionData[]` */
export function normalizeGraphConnections(connections: unknown): ConnectionData[] {
  return connectionItemsFromUnknown(connections).map(connectionItemToConnectionData);
}

/** 兼容后端/历史快照中的多种 connections 形态 → domain `ConnectionItem[]` */
export function connectionItemsFromUnknown(connections: unknown): ConnectionItem[] {
  if (!connections) return [];
  if (Array.isArray(connections)) {
    return connections.flatMap((item) => {
      if (typeof item !== 'object' || item === null) return [];
      if ('fromPin' in item && 'toPin' in item) {
        const row = item as ConnectionItem;
        return typeof row.fromPin === 'string' && typeof row.toPin === 'string' ? [row] : [];
      }
      if ('from' in item && 'to' in item) {
        const row = item as { from: string; to: string };
        return typeof row.from === 'string' && typeof row.to === 'string'
          ? [{ fromPin: row.from, toPin: row.to }]
          : [];
      }
      return [];
    });
  }
  if (typeof connections === 'object' && connections !== null) {
    if ('connections' in connections && Array.isArray((connections as Connection).connections)) {
      return (connections as Connection).connections;
    }
    return Object.values(connections).flatMap((value) => {
      if (Array.isArray(value) && value.length === 2 && typeof value[0] === 'string') {
        return [{ fromPin: value[0], toPin: value[1] }];
      }
      if (typeof value === 'object' && value !== null && 'fromPin' in value && 'toPin' in value) {
        return [value as ConnectionItem];
      }
      return [];
    });
  }
  return [];
}

export function pinDataToDomainPin(pin: PinData): Pin {
  return {
    id: pin.id,
    nodeId: pin.nodeId,
    name: pin.name,
    type: pin.type as PinType,
    direction: pin.direction,
    defaultValue: pin.defaultValue,
    userValue: pin.userValue,
    containerType: pin.containerType,
    typeDisplay: pin.typeDisplay,
    dataType: pin.dataType,
    optional: pin.optional,
    ui: pin.ui,
  };
}

export function domainPinToPinData(pin: Pin): PinData {
  return {
    id: pin.id,
    nodeId: pin.nodeId,
    name: pin.name,
    type: pin.type,
    direction: pin.direction,
    defaultValue: pin.defaultValue,
    userValue: pin.userValue,
    containerType: pin.containerType,
    typeDisplay: pin.typeDisplay,
    dataType: pin.dataType,
    optional: pin.optional,
    ui: pin.ui,
  };
}

function resolveDomainNodes(nodes: NodeData[], pinMap: Map<string, Pin>): Graph['nodes'] {
  return nodes.map((node) => ({
    id: node.id,
    nodeType: node.nodeType,
    category: node.category,
    title: node.title,
    inputs: node.inputs
      .map((pinId) => pinMap.get(pinId))
      .filter((pin): pin is Pin => pin != null),
    outputs: node.outputs
      .map((pinId) => pinMap.get(pinId))
      .filter((pin): pin is Pin => pin != null),
    uiStyle: node.uiStyle,
    description: node.description,
    position: node.position,
    paramsKind: node.paramsKind,
    variableId: node.variableId,
    variableName: node.variableName,
    variableType: node.variableType,
    subGraphId: node.subGraphId,
    dataframeId: node.dataframeId,
    isInternal: node.isInternal,
  })) as Graph['nodes'];
}

/** Store 图 → domain Graph（ProjectData 快照） */
export function graphDataToDomainGraph(data: GraphData): Graph {
  const domainPins = data.pins.map(pinDataToDomainPin);
  const pinMap = new Map(domainPins.map((pin) => [pin.id, pin]));

  return {
    id: data.id,
    name: data.name,
    type: data.type,
    functionInputs: data.functionInputs,
    functionOutputs: data.functionOutputs,
    nodes: resolveDomainNodes(data.nodes, pinMap),
    pins: domainPins,
    connections: { connections: connectionDataToItems(data.connections) },
    canvas: data.canvas,
  };
}

function domainNodeToNodeData(graphId: string, node: DomainGraphNode): NodeData {
  return {
    id: node.id,
    graphId,
    nodeType: node.nodeType,
    category: node.category,
    title: node.title,
    inputs: node.inputs.map((pin) => pin.id),
    outputs: node.outputs.map((pin) => pin.id),
    uiStyle: node.uiStyle,
    description: node.description,
    position: node.position ?? { x: 0, y: 0 },
    paramsKind: node.paramsKind,
    variableId: node.variableId,
    variableName: node.variableName,
    variableType: node.variableType,
    subGraphId: node.subGraphId,
    dataframeId: node.dataframeId,
    isInternal: node.isInternal,
  };
}

/** domain Graph → Store 图（hydrate 入口规范化） */
export function domainGraphToGraphData(graph: Graph): GraphData {
  const pins = graph.pins.map(domainPinToPinData);
  return {
    id: graph.id,
    name: graph.name,
    type: graph.type,
    functionInputs: graph.functionInputs,
    functionOutputs: graph.functionOutputs,
    nodes: graph.nodes.map((node) => domainNodeToNodeData(graph.id, node as DomainGraphNode)),
    pins,
    connections: normalizeGraphConnections(graph.connections),
    canvas: graph.canvas,
  };
}

export function graphDataRecordToDomainGraphs(
  graphs: Record<string, GraphData>,
): Record<string, Graph> {
  return Object.fromEntries(
    Object.entries(graphs).map(([id, graph]) => [id, graphDataToDomainGraph(graph)]),
  );
}

export function domainGraphRecordToGraphData(
  graphs: Record<string, Graph>,
): Record<string, GraphData> {
  return Object.fromEntries(
    Object.entries(graphs).map(([id, graph]) => [id, domainGraphToGraphData(graph)]),
  );
}

/** GraphInstanceDTO → GraphData（IPC 入站；委托 `normalizeGraphDataLike` 单点） */
export function graphInstanceDtoToGraphData(dto: GraphInstanceDTO): GraphData {
  return normalizeGraphDataLike(dto.id, {
    ...dto,
    canvas:
      dto.canvas ??
      (dto as GraphInstanceDTO & { position?: GraphData['canvas'] }).position,
  });
}

function resolveGraphType(graph: GraphDataLike): GraphData['type'] {
  const raw = (graph as GraphData).type ?? (graph as Graph).type;
  if (typeof raw === 'string') {
    return raw.toLowerCase() as GraphData['type'];
  }
  return String(raw).toLowerCase() as GraphData['type'];
}

function resolveGraphNodes(graph: GraphDataLike): RuntimeNodeInput[] {
  return (graph.nodes ?? []) as RuntimeNodeInput[];
}

function resolveGraphPins(graph: GraphDataLike): PinData[] {
  return (graph.pins ?? []) as PinData[];
}

/** 单个运行时 pin 引用 → PinId */
export function runtimePinRefToId(pin: string | PinData | import('../store/graph').PinView): string {
  return typeof pin === 'string' ? pin : (pin?.id ?? '');
}

/** `RuntimeNodeInput.inputs/outputs` → PinId[]（hydrate 共用） */
export function runtimePinRefsToIds(arr: unknown): string[] {
  if (!Array.isArray(arr)) return [];
  return arr.map((pin) => runtimePinRefToId(pin as string | PinData)).filter(Boolean);
}

function runtimeNodeInputToNodeData(graphId: string, node: RuntimeNodeInput): NodeData {
  const stored = node as NodeData;
  return {
    ...(node as NodeData),
    graphId,
    nodeType: stored.nodeType ?? node.nodeType ?? '',
    category: stored.category ?? [],
    title: stored.title ?? '',
    uiStyle: stored.uiStyle ?? 'default',
    position: stored.position ?? { x: 0, y: 0 },
    inputs: runtimePinRefsToIds(node.inputs),
    outputs: runtimePinRefsToIds(node.outputs),
  };
}

/** Graph 同步事件 patch → `GraphDataLike`（`GraphEventHandler` 入站） */
export function graphUpdatedPayloadToGraphDataLike(
  graphId: string,
  kind: 'event' | 'function',
  fallbackName: string,
  data: Partial<Graph> & {
    functionInputs?: GraphData['functionInputs'];
    functionOutputs?: GraphData['functionOutputs'];
  },
): GraphDataLike {
  return {
    id: graphId,
    name: data.name ?? fallbackName,
    type: kind,
    ...data,
    nodes: data.nodes ?? [],
    pins: data.pins ?? [],
    connections: data.connections ?? { connections: [] },
    canvas: data.canvas ?? { x: 0, y: 0, scale: 1 },
  };
}

/** hydrate 入口：将 `GraphDataLike` 规范化为 store 权威 `GraphData` */
export function normalizeGraphDataLike(graphId: string, graph: GraphDataLike): GraphData {
  const graphType = resolveGraphType(graph);
  const name = (graph as GraphData).name ?? (graph as Graph).name ?? graphId;

  return {
    id: graphId,
    name,
    type: graphType,
    functionInputs: (graph as GraphData).functionInputs ?? (graph as Graph).functionInputs,
    functionOutputs: (graph as GraphData).functionOutputs ?? (graph as Graph).functionOutputs,
    nodes: resolveGraphNodes(graph).map((node) => runtimeNodeInputToNodeData(graphId, node)),
    pins: resolveGraphPins(graph),
    connections: normalizeGraphConnections((graph as GraphDataLike & { connections?: unknown }).connections),
    canvas:
      (graph as GraphData).canvas ??
      (graph as Graph).canvas ??
      (graph as GraphInstanceDTO & { position?: GraphData['canvas'] }).position ??
      { x: 0, y: 0, scale: 1 },
  };
}
