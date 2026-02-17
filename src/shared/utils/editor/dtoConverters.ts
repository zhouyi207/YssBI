/**
 * DTO 转换工具函数
 *
 * 提供前后端数据转换的实用工具
 * 统一处理 DTO 与 Store 格式的互转
 */

import type {
  Node,
  Pin,
  Graph,
  Connection,
  ProjectData,
} from '@/shared/types/domain';

import type { ConnectionItemDTO, GraphInstanceDTO, NodeInstanceDTO } from '@/shared/types/dto/graph';
import type { ConnectionData } from '@/shared/types/store/graph';
import type { ProjectDataDTO } from '@/shared/types/dto';

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

/**
 * 将后端返回的 Graph DTO 转换为前端 Graph 对象
 * 处理 nodes 和 pins 的关联关系
 */
export function convertGraphFromDTO(graphDTO: GraphInstanceDTO): Graph {
  const { nodes, pins, ...rest } = graphDTO;

  // 创建 Pin ID 到 Pin 对象的映射
  const pinMap = new Map<string, Pin>();
  pins.forEach((pin) => {
    pinMap.set(pin.id, pin as Pin);
  });

  // 为每个节点关联其 inputs 和 outputs
  const convertedNodes = nodes.map((node: NodeInstanceDTO) => {
    const inputPins = (node.inputs || [])
      .map((pinId: string) => pinMap.get(pinId))
      .filter(Boolean);
    
    const outputPins = (node.outputs || [])
      .map((pinId: string) => pinMap.get(pinId))
      .filter(Boolean);

    return {
      ...node,
      inputs: inputPins,
      outputs: outputPins,
    };
  });

  return {
    ...rest,
    nodes: convertedNodes,
    pins,
  };
}

/**
 * 将前端 Graph 对象转换为后端 DTO
 * 将 Pin 对象转换为 Pin ID 列表
 */
export function convertGraphToDTO(graph: Graph): GraphInstanceDTO {
  const { nodes, ...rest } = graph;

  const convertedNodes: NodeInstanceDTO[] = nodes.map((node) => {
    const nodeWithPos = node as Node & { position?: { x: number; y: number } };
    return {
      id: node.id,
      nodeType: node.node_type,
      category: node.category,
      title: node.title,
      inputs: node.inputs.map(pin => pin.id),
      outputs: node.outputs.map(pin => pin.id),
      uiStyle: node.ui_style,
      description: node.description,
      position: nodeWithPos.position ?? { x: 0, y: 0 },
    };
  });

  return {
    ...rest,
    nodes: convertedNodes,
  };
}

/**
 * 批量转换 Graphs
 */
export function convertGraphsFromDTO(
  graphsDTO: Record<string, GraphInstanceDTO>
): Record<string, Graph> {
  const result: Record<string, Graph> = {};
  
  for (const [id, graphDTO] of Object.entries(graphsDTO)) {
    result[id] = convertGraphFromDTO(graphDTO);
  }
  
  return result;
}

/**
 * 批量转换 Graphs 到 DTO
 */
export function convertGraphsToDTO(
  graphs: Record<string, Graph>
): Record<string, GraphInstanceDTO> {
  const result: Record<string, GraphInstanceDTO> = {};
  
  for (const [id, graph] of Object.entries(graphs)) {
    result[id] = convertGraphToDTO(graph);
  }
  
  return result;
}

/**
 * 转换 ProjectData 从 DTO
 */
export function convertProjectDataFromDTO(dto: ProjectDataDTO): ProjectData {
  return {
    ...dto,
    graphs: convertGraphsFromDTO(dto.graphs),
  };
}

/**
 * 转换 ProjectData 到 DTO
 */
export function convertProjectDataToDTO(data: ProjectData): ProjectDataDTO {
  return {
    ...data,
    graphs: convertGraphsToDTO(data.graphs),
  };
}

/**
 * 从 Connection DTO 构建 Pin 的 links 关系
 * 更新 Pin 对象的 links 数组
 */
export function applyConnectionsToPins(
  pins: Pin[],
  connections: Connection
): void {
  // 清空所有 links
  pins.forEach(pin => {
    pin.links = [];
  });

  // 创建 Pin ID 到 Pin 对象的映射
  const pinMap = new Map<string, Pin>();
  pins.forEach(pin => {
    pinMap.set(pin.id, pin);
  });

  // 应用连接关系（camelCase: fromPin, toPin）
  connections.connections.forEach((conn: { fromPin: string; toPin: string }) => {
    const fromId = conn.fromPin;
    const toId = conn.toPin;
    const fromPin = fromId ? pinMap.get(fromId) : undefined;
    const toPin = toId ? pinMap.get(toId) : undefined;

    if (fromPin && toPin) {
      if (!fromPin.links.includes(toId!)) {
        fromPin.links.push(toId!);
      }
      if (!toPin.links.includes(fromId!)) {
        toPin.links.push(fromId!);
      }
    }
  });
}

/**
 * 从 Pins 的 links 构建 Connection DTO
 */
export function extractConnectionsFromPins(pins: Pin[]): Connection {
  const connections: { fromPin: string; toPin: string }[] = [];
  const seen = new Set<string>();

  pins.forEach(pin => {
    // 只处理输出 Pin 的连接
    if (pin.direction === 'output') {
      pin.links.forEach(targetPinId => {
        const key = `${pin.id}->${targetPinId}`;
        if (!seen.has(key)) {
          connections.push({
            fromPin: pin.id,
            toPin: targetPinId,
          });
          seen.add(key);
        }
      });
    }
  });

  return { connections };
}

/**
 * 验证 DTO 数据的完整性
 */
export function validateGraphDTO(graphDTO: GraphInstanceDTO): {
  valid: boolean;
  errors: string[];
} {
  const errors: string[] = [];

  if (!graphDTO.id) {
    errors.push('Missing graph id');
  }

  if (!graphDTO.name) {
    errors.push('Missing graph name');
  }

  if (!graphDTO.type) {
    errors.push('Missing graph type');
  }

  if (!Array.isArray(graphDTO.nodes)) {
    errors.push('Invalid nodes array');
  }

  if (!Array.isArray(graphDTO.pins)) {
    errors.push('Invalid pins array');
  }

  if (!graphDTO.connections || !Array.isArray(graphDTO.connections.connections)) {
    errors.push('Invalid connections structure');
  }

  return {
    valid: errors.length === 0,
    errors,
  };
}

/**
 * 深度克隆 DTO 对象
 */
export function cloneDTO<T>(dto: T): T {
  return JSON.parse(JSON.stringify(dto));
}

/**
 * 合并两个 ProjectData，用于增量更新
 */
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
