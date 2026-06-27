/**
 * Graph DTO 转换
 *
 * 前后端 Graph/Node/Connection 格式互转
 */

import type { Node, Pin, Graph, ProjectData } from '../domain';
import type {
  ConnectionItemDTO,
  GraphInstanceDTO,
  NodeInstanceDTO,
} from './graph';
import type { ConnectionData } from '../store/graph';
import type { ProjectDataDTO } from './api';

/** 将 ConnectionItemDTO 转为 Store 的 ConnectionData */
export function connectionItemToConnectionData(
  item: ConnectionItemDTO
): ConnectionData {
  const from = item.fromPin;
  const to = item.toPin;
  return { id: `${from}->${to}`, from, to };
}

/** 将 ConnectionData 转为 ConnectionItemDTO */
export function connectionDataToItem(conn: ConnectionData): ConnectionItemDTO {
  return { fromPin: conn.from, toPin: conn.to };
}

/** 将后端 Graph DTO 转换为前端 Graph 对象 */
export function convertGraphFromDTO(graphDTO: GraphInstanceDTO): Graph {
  const { nodes, pins, ...rest } = graphDTO;

  const pinMap = new Map<string, Pin>();
  pins.forEach((pin) => {
    pinMap.set(pin.id, { ...pin, links: [] } as Pin);
  });
  graphDTO.connections.connections.forEach(({ fromPin, toPin }) => {
    const from = pinMap.get(fromPin);
    const to = pinMap.get(toPin);
    if (from && !from.links.includes(toPin)) from.links.push(toPin);
    if (to && !to.links.includes(fromPin)) to.links.push(fromPin);
  });

  const convertedNodes = nodes.map((node: NodeInstanceDTO) => {
    const inputPins = (node.inputs || [])
      .map((pinId: string) => pinMap.get(pinId))
      .filter(Boolean);

    const outputPins = (node.outputs || [])
      .map((pinId: string) => pinMap.get(pinId))
      .filter(Boolean);

    return {
      ...node,
      nodeType: node.nodeType ?? '',
      uiStyle: node.uiStyle ?? 'default',
      inputs: inputPins as Pin[],
      outputs: outputPins as Pin[],
    } as Node;
  });

  return {
    ...rest,
    nodes: convertedNodes as Node[],
    pins: Array.from(pinMap.values()),
  };
}

/** 将前端 Graph 对象转换为后端 DTO */
export function convertGraphToDTO(graph: Graph): GraphInstanceDTO {
  const { nodes, ...rest } = graph;

  const convertedNodes: NodeInstanceDTO[] = nodes.map((node) => {
    const nodeWithPos = node as Node & { position?: { x: number; y: number } };
    return {
      id: node.id,
      nodeType: node.nodeType,
      category: node.category,
      title: node.title,
      inputs: node.inputs.map((pin) => pin.id),
      outputs: node.outputs.map((pin) => pin.id),
      uiStyle: node.uiStyle,
      description: node.description,
      position: nodeWithPos.position ?? { x: 0, y: 0 },
    };
  });

  return {
    ...rest,
    nodes: convertedNodes,
  };
}

/** 批量转换 Graphs */
export function convertGraphsFromDTO(
  graphsDTO: Record<string, GraphInstanceDTO>
): Record<string, Graph> {
  const result: Record<string, Graph> = {};
  for (const [id, graphDTO] of Object.entries(graphsDTO)) {
    result[id] = convertGraphFromDTO(graphDTO);
  }
  return result;
}

/** 批量转换 Graphs 到 DTO */
export function convertGraphsToDTO(
  graphs: Record<string, Graph>
): Record<string, GraphInstanceDTO> {
  const result: Record<string, GraphInstanceDTO> = {};
  for (const [id, graph] of Object.entries(graphs)) {
    result[id] = convertGraphToDTO(graph);
  }
  return result;
}

/** 转换 ProjectData 从 DTO */
export function convertProjectDataFromDTO(dto: ProjectDataDTO): ProjectData {
  return {
    ...dto,
    graphs: convertGraphsFromDTO(dto.graphs),
  };
}

/** 转换 ProjectData 到 DTO */
export function convertProjectDataToDTO(
  data: ProjectData
): ProjectDataDTO {
  return {
    ...data,
    graphs: convertGraphsToDTO(data.graphs),
  };
}

/** 验证 Graph DTO */
export function validateGraphDTO(graphDTO: GraphInstanceDTO): {
  valid: boolean;
  errors: string[];
} {
  const errors: string[] = [];

  if (!graphDTO.id) errors.push('Missing graph id');
  if (!graphDTO.name) errors.push('Missing graph name');
  if (!graphDTO.type) errors.push('Missing graph type');
  if (!Array.isArray(graphDTO.nodes)) errors.push('Invalid nodes array');
  if (!Array.isArray(graphDTO.pins)) errors.push('Invalid pins array');
  if (
    !graphDTO.connections ||
    !Array.isArray(graphDTO.connections.connections)
  ) {
    errors.push('Invalid connections structure');
  }

  return { valid: errors.length === 0, errors };
}

/** 深度克隆 DTO */
export function cloneDTO<T>(dto: T): T {
  return JSON.parse(JSON.stringify(dto));
}

/** 合并 ProjectData */
export function mergeProjectData(
  base: ProjectData,
  updates: Partial<ProjectData>
): ProjectData {
  return {
    variables: { ...base.variables, ...updates.variables },
    graphs: { ...base.graphs, ...updates.graphs },
    databases: { ...base.databases, ...updates.databases },
    metadata: updates.metadata || base.metadata,
  };
}
