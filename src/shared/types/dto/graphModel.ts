/**
 * GraphData（store）→ domain Graph 显式转换。
 * 项目快照导出边界只接受规范化的 store 数据。
 */

import type { Graph } from '../domain/graph';
import type { Pin } from '../domain/pin';
import { connectionDataToItems } from './graphConverters';
import type { GraphData, NodeData, PinData } from '../store/graph';

function pinDataToDomainPin(pin: PinData): Pin {
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
    validationWarning: pin.validationWarning,
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
    description: node.description,
    position: node.position,
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
  };
}

export function graphDataRecordToDomainGraphs(
  graphs: Record<string, GraphData>,
): Record<string, Graph> {
  return Object.fromEntries(
    Object.entries(graphs).map(([id, graph]) => [id, graphDataToDomainGraph(graph)]),
  );
}
