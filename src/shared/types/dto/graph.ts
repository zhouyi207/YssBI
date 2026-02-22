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

/** Tagged enum 判别字段 */
export type ParamsKind = 'none' | 'variable' | 'subGraph' | 'dataFrame';

/** 后端 NodeInstanceDTO 对应（camelCase），instance_params 通过 flatten 展开到顶层 */
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
  /** 参数类型判别字段 */
  paramsKind: ParamsKind;
  /** Variable 变体 */
  variableId?: string;
  variableName?: string;
  variableType?: string;
  /** SubGraph 变体 */
  subGraphId?: string;
  /** DataFrame 变体 */
  dataframeId?: string;
  dataframeName?: string;
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
  containerType?: string;
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
