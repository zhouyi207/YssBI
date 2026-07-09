/**
 * Pin DTO → store 规范化：视觉/展示字段由前端从 `dataType` 本地推导。
 */

import type { DataType } from '../domain/dataType';
import type { PinInstanceDTO } from './graph';
import { dataTypeFromBackend, type DataTypeBackendFormat } from './dataType';
import type { PinData } from '../store/graph';

function resolveDomainDataType(
  dataType: DataType | DataTypeBackendFormat | undefined,
): DataType | undefined {
  if (!dataType) return undefined;
  if (typeof dataType === 'object' && 'kind' in dataType) {
    return dataType as DataType;
  }
  return dataTypeFromBackend(dataType as DataTypeBackendFormat);
}

/** 后端 PinInstanceDTO / 历史 PinData → store 权威形态 */
export function normalizePinDto(pin: PinInstanceDTO | PinData): PinData {
  const base = {
    id: pin.id,
    nodeId: pin.nodeId,
    name: pin.name,
    direction: pin.direction,
    defaultValue: pin.defaultValue,
    userValue: pin.userValue,
    optional: pin.optional,
    ui: pin.ui,
  };

  if (pin.type === 'exec') {
    return { ...base, type: 'exec' };
  }

  const dataType = resolveDomainDataType(pin.dataType);

  return {
    ...base,
    type: 'object',
    dataType,
  };
}

/** `PinTypesInferred` 事件 patch：仅更新结构化类型 */
export function pinInferredPatch(dataType: DataType): Partial<PinData> {
  return {
    dataType,
    type: 'object',
  };
}
