/**
 * GraphData（store）↔ domain Graph 显式转换
 * 项目快照导出 / hydrate 边界单点，避免 `as unknown as` 漂移。
 */

import type { Connection, ConnectionItem } from '../domain/connection';
import type { Graph } from '../domain/graph';
import type { Pin } from '../domain/pin';
import {
  connectionDataToItems,
  connectionItemToConnectionData,
} from './graphConverters';
import { normalizePinDto } from './pinHydrate';
import type { GraphInstanceDTO } from './graph';
import type {
  ConnectionData,
  GraphData,
  GraphDataLike,
  NodeData,
  PinData,
  RuntimeNodeInput,
} from '../store/graph';

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
    type: pin.type,
    direction: pin.direction,
    defaultValue: pin.defaultValue,
    userValue: pin.userValue,
    dataType: pin.dataType,
    optional: pin.optional,
    ui: pin.ui,
  };
}

export function domainPinToPinData(pin: Pin): PinData {
  return normalizePinDto({
    id: pin.id,
    nodeId: pin.nodeId,
    name: pin.name,
    type: pin.type,
    direction: pin.direction,
    defaultValue: pin.defaultValue,
    userValue: pin.userValue,
    dataType: pin.dataType,
    optional: pin.optional,
    ui: pin.ui,
  });
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
    description: node.description,
    position: node.position,
    paramsKind: node.paramsKind,
    variableId: node.variableId,
    variableName: node.variableName,
    variableType: node.variableType,
    subGraphPath: node.subGraphPath,
    dataframeId: node.dataframeId,
    isInternal: node.isInternal,
  })) as Graph['nodes'];
}

/** Store 图 → domain Graph（ProjectData 快照） */
export function graphDataToDomainGraph(data: GraphData): Graph {
  const domainPins = data.pins.map(pinDataToDomainPin);
  const pinMap = new Map(domainPins.map((pin) => [pin.id, pin]));

  return {
    path: data.path,
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

/** domain Graph → Store 图（结构转换；注册表 enrich 在 `graphDataStore` hydrate 边界） */
export function domainGraphToGraphData(graph: Graph): GraphData {
  return normalizeGraphDataLike(graph.path, graph);
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
  return normalizeGraphDataLike(dto.path, {
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
  return (graph.pins ?? []).map((pin) => normalizePinDto(pin as PinData));
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

function runtimeNodeInputToNodeData(graphPath: string, node: RuntimeNodeInput): NodeData {
  const stored = node as NodeData;
  const nodeType = stored.nodeType ?? node.nodeType ?? '';
  return {
    ...(node as NodeData),
    graphPath,
    nodeType,
    category: stored.category ?? node.category ?? [],
    title: stored.title ?? node.title ?? nodeType,
    position: stored.position ?? { x: 0, y: 0 },
    inputs: runtimePinRefsToIds(node.inputs),
    outputs: runtimePinRefsToIds(node.outputs),
  };
}

/** Graph 同步事件 patch → `GraphDataLike`（`GraphEventHandler` 入站） */
export function graphUpdatedPayloadToGraphDataLike(
  graphPath: string,
  kind: 'event' | 'function',
  fallbackName: string,
  data: Partial<Graph> & {
    functionInputs?: GraphData['functionInputs'];
    functionOutputs?: GraphData['functionOutputs'];
  },
): GraphDataLike {
  return {
    path: graphPath,
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
export function normalizeGraphDataLike(graphPath: string, graph: GraphDataLike): GraphData {
  const graphType = resolveGraphType(graph);
  const name = (graph as GraphData).name ?? (graph as Graph).name ?? graphPath;

  return {
    path: graphPath,
    name,
    type: graphType,
    functionInputs: (graph as GraphData).functionInputs ?? (graph as Graph).functionInputs,
    functionOutputs: (graph as GraphData).functionOutputs ?? (graph as Graph).functionOutputs,
    nodes: resolveGraphNodes(graph).map((node) => runtimeNodeInputToNodeData(graphPath, node)),
    pins: resolveGraphPins(graph),
    connections: normalizeGraphConnections((graph as GraphDataLike & { connections?: unknown }).connections),
    canvas:
      (graph as GraphData).canvas ??
      (graph as Graph).canvas ??
      (graph as GraphInstanceDTO & { position?: GraphData['canvas'] }).position ??
      { x: 0, y: 0, scale: 1 },
  };
}
