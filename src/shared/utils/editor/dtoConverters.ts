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
} from '@/shared/types/domain';

import type { ProjectData } from '@/shared/types/domain';
import type { ConnectionItemDTO } from '@/shared/types/dto/graph';
import type { ConnectionData } from '@/shared/types/store/graph';

/** 将 ConnectionItemDTO 转为 Store 的 ConnectionData */
export function connectionItemToConnectionData(
  item: ConnectionItemDTO
): ConnectionData {
  const from = item.from_pin;
  const to = item.to_pin;
  return { id: `${from}->${to}`, from, to };
}

/** 将 ConnectionData 转为 ConnectionItemDTO */
export function connectionDataToItem(conn: ConnectionData): ConnectionItemDTO {
  return { from_pin: conn.from, to_pin: conn.to };
}

/**
 * 将后端返回的 Graph DTO 转换为前端 Graph 对象
 * 处理 nodes 和 pins 的关联关系
 */
export function convertGraphFromDTO(graphDTO: any): Graph {
  const { nodes, pins, ...rest } = graphDTO;

  // 创建 Pin ID 到 Pin 对象的映射
  const pinMap = new Map<string, Pin>();
  pins.forEach((pin: Pin) => {
    pinMap.set(pin.id, pin);
  });

  // 为每个节点关联其 inputs 和 outputs
  const convertedNodes = nodes.map((node: any) => {
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
export function convertGraphToDTO(graph: Graph): any {
  const { nodes, ...rest } = graph;

  const convertedNodes = nodes.map((node: Node) => ({
    ...node,
    inputs: node.inputs.map(pin => pin.id),
    outputs: node.outputs.map(pin => pin.id),
  }));

  return {
    ...rest,
    nodes: convertedNodes,
  };
}

/**
 * 批量转换 Graphs
 */
export function convertGraphsFromDTO(
  graphsDTO: Record<string, any>
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
): Record<string, any> {
  const result: Record<string, any> = {};
  
  for (const [id, graph] of Object.entries(graphs)) {
    result[id] = convertGraphToDTO(graph);
  }
  
  return result;
}

/**
 * 转换 ProjectData 从 DTO
 */
export function convertProjectDataFromDTO(dto: any): ProjectData {
  return {
    ...dto,
    graphs: convertGraphsFromDTO(dto.graphs),
  };
}

/**
 * 转换 ProjectData 到 DTO
 */
export function convertProjectDataToDTO(data: ProjectData): any {
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

  // 应用连接关系
  connections.connections.forEach(conn => {
    const fromPin = pinMap.get(conn.from_pin);
    const toPin = pinMap.get(conn.to_pin);

    if (fromPin && toPin) {
      // 输出 Pin 记录所有连接的目标
      if (!fromPin.links.includes(conn.to_pin)) {
        fromPin.links.push(conn.to_pin);
      }
      
      // 输入 Pin 记录来源（通常只有一个）
      if (!toPin.links.includes(conn.from_pin)) {
        toPin.links.push(conn.from_pin);
      }
    }
  });
}

/**
 * 从 Pins 的 links 构建 Connection DTO
 */
export function extractConnectionsFromPins(pins: Pin[]): Connection {
  const connections: { from_pin: string; to_pin: string }[] = [];
  const seen = new Set<string>();

  pins.forEach(pin => {
    // 只处理输出 Pin 的连接
    if (pin.direction === 'output') {
      pin.links.forEach(targetPinId => {
        const key = `${pin.id}->${targetPinId}`;
        if (!seen.has(key)) {
          connections.push({
            from_pin: pin.id,
            to_pin: targetPinId,
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
export function validateGraphDTO(graphDTO: any): {
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
