/**
 * DTO 类型定义
 * 与后端 Rust 结构 1:1 对应，用于前后端数据传输
 * 序列化时后端使用 snake_case，前端接收后保持 JSON 原始格式
 */

import type { GraphPosition } from '../domain/graph';
import type { PinDirection } from '../domain/pin';

// ==================== Node DTO ====================

export interface NodePositionDTO {
  x: number;
  y: number;
}

/** 后端 NodeInstanceDTO 对应（camelCase） */
export interface NodeInstanceDTO {
  id: string;
  nodeType: string;
  category: string[];
  title: string;
  inputs: string[];  // Pin IDs
  outputs: string[]; // Pin IDs
  uiStyle: string;
  description?: string;
  position: NodePositionDTO;
  /** 变量节点：绑定的变量 ID */
  variableId?: string;
  variableName?: string;
  variableType?: string;
  /** 函数/宏调用节点：子图 ID */
  subGraphId?: string;
  /** DataFrame 节点：数据帧 ID */
  dataframeId?: string;
  /** Get Column 节点：列名、列类型 */
  columnName?: string;
  columnType?: string;
}

// ==================== Pin DTO ====================

export interface PinUIDTO {
  x?: number;
  y?: number;
  color?: string;
}

/** 后端 PinInstanceDTO 对应（后端使用 rename_all = "camelCase" 序列化） */
export interface PinInstanceDTO {
  id: string;
  nodeId: string;
  name: string;
  type: string;
  direction: PinDirection;
  links: string[];
  defaultValue?: unknown;
  userValue?: unknown;
  isArray?: boolean;
  ui?: PinUIDTO;
}

// ==================== Connection DTO ====================

/** 后端 ConnectionItemDTO 对应（camelCase） */
export interface ConnectionItemDTO {
  fromPin: string;
  toPin: string;
}

/** 后端 ConnectionDTO 对应 */
export interface ConnectionDTO {
  connections: ConnectionItemDTO[];
}

// ==================== Graph DTO ====================

export type GraphTypeDTO = 'event' | 'function' | 'macro';

/** 后端 GraphInstanceDTO 对应 */
export interface GraphInstanceDTO {
  id: string;
  name: string;
  type: GraphTypeDTO;
  nodes: NodeInstanceDTO[];
  pins: PinInstanceDTO[];
  connections: ConnectionDTO;
  canvas: GraphPosition;
}
