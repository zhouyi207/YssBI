/**
 * Graph / Connection DTO 转换
 */

import type {
  ConnectionItemDTO,
  GraphInstanceDTO,
} from './graph';
import type { ConnectionData } from '../store/graph';
import type { ProjectData } from '../domain';

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
