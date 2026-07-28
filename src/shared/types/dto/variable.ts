/**
 * Variable DTO
 *
 * 与后端 VariableInstanceDTO 对应，含前后端转换逻辑
 */

import type { Variable, VariableScope } from '../domain/variable';
import type { DataType, DataValue } from '../domain';
import type { DataTypeBackendFormat } from './dataType';
import type { DataValueBackend } from './dataValue';
import { dataTypeFromBackend } from './dataType';
import { dataValueFromBackend } from './dataValue';

export type { DataValueBackend } from './dataValue';

/** 变量实例 DTO - 与后端 VariableInstanceDTO 一一对应 */
export interface VariableInstanceDTO {
  id: string;
  name: string;
  dataType: DataType;
  dataValue: DataValue;
  description: string;
  scope: VariableScope;
  tags: string[];
}

/** 后端变量 payload 原始格式（dataType/dataValue 可能为后端格式） */
type VariablePayloadRaw = Omit<VariableInstanceDTO, 'dataType' | 'dataValue'> & {
  dataType?: unknown;
  dataValue?: unknown;
};

/** 将后端变量 payload 规范化为前端 Variable */
export function normalizeVariableFromBackend(raw: VariablePayloadRaw): Variable {
  const dataType =
    raw.dataType && typeof raw.dataType === 'object' && 'kind' in raw.dataType
      ? (raw.dataType as Variable['dataType'])
      : dataTypeFromBackend(raw.dataType as DataTypeBackendFormat);

  const dataValue = dataValueFromBackend(raw.dataValue as DataValueBackend);

  if (!Number.isSafeInteger(raw.revision) || (raw.revision as number) < 0) {
    throw new Error(`variable '${raw.id}' revision is missing or malformed`);
  }

  return {
    id: raw.id,
    revision: raw.revision as number,
    name: raw.name,
    dataType,
    dataValue,
    description: raw.description ?? '',
    scope: raw.scope ?? { type: 'global' },
    tags: raw.tags ?? [],
  };
}
