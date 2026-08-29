/**
 * Variable DTO
 *
 * 与后端 VariableInstanceDTO 对应，含前后端转换逻辑
 */

import type { VariableScope } from '../domain/variable';
import type { DataType, DataValue } from '../domain';

/** 变量实例 DTO - 与后端 VariableInstanceDTO 一一对应 */
export interface VariableInstanceDTO {
  id: string;
  resourcePath?: string;
  name: string;
  dataType: DataType;
  dataValue: DataValue;
  description: string;
  scope: VariableScope;
  tags: string[];
}

export { normalizeVariableFromBackend } from '../domain/variable';
