/**
 * DTO 类型定义
 * 与后端 Rust 结构 1:1 对应，用于前后端数据传输
 * 序列化时后端使用 snake_case，前端接收后保持 JSON 原始格式
 */

import type { PinDirection } from "../domain/pin";
import type { DataType } from "../domain/dataType";

// ==================== Node DTO ====================

export interface NodePositionDTO {
  x: number;
  y: number;
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
  defaultValue?: unknown;
  userValue?: unknown;
  dataType?: DataType;
  optional?: boolean;
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

export interface GraphValidationWarningDTO {
  code: string;
  fromPinId: string;
  toPinId: string;
  message: string;
}

export interface FunctionCallSiteDTO {
  callerGraphPath: string;
  nodeIds: string[];
}
