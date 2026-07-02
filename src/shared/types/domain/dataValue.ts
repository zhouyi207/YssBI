/**
 * Domain Types - DataValue
 *
 * 数据值定义及领域内辅助函数
 */

import type { DataType } from './dataType';
import { getDefaultValue } from './dataType';

export type DataSeriesValuePayload = {
  id: string;
  elementType?: import('./dataType').DataType;
};

/**
 * 数据值（可辨识联合，与 DataType 对应）
 */
export type DataValue =
  | { kind: 'Boolean'; value: boolean }
  | { kind: 'Int32'; value: number }
  | { kind: 'Int64'; value: number }
  | { kind: 'Float32'; value: number }
  | { kind: 'Float64'; value: number }
  | { kind: 'String'; value: string }
  | { kind: 'Date'; value: string }
  | { kind: 'Categorical'; value: string }
  | { kind: 'Array'; value: DataValue[] }
  | { kind: 'Object'; value: Record<string, unknown> }
  | { kind: 'DataFrame'; value: string }
  | { kind: 'DataSeries'; value: DataSeriesValuePayload | string }
  | { kind: 'Null' };

/** 从 DataValue 提取原始值（用于 UI 显示/编辑） */
export function dataValueToRaw(dv: DataValue): unknown {
  switch (dv.kind) {
    case 'Boolean':
    case 'Int32':
    case 'Int64':
    case 'Float32':
    case 'Float64':
    case 'String':
    case 'Date':
    case 'Categorical':
    case 'DataFrame':
      return dv.value;
    case 'DataSeries':
      return typeof dv.value === 'string' ? dv.value : dv.value.id;
    case 'Array':
      return dv.value.map(dataValueToRaw);
    case 'Object':
      return dv.value;
    case 'Null':
      return null;
  }
}

/** 从原始值 + DataType 创建 DataValue */
export function dataValueFromRaw(raw: unknown, dataType: DataType): DataValue {
  const def = getDefaultValue(dataType);
  if (raw === null || raw === undefined) {
    return rawToDataValue(def, dataType);
  }
  return rawToDataValue(raw, dataType);
}

function rawToDataValue(raw: unknown, dataType: DataType): DataValue {
  const k = dataType.kind;
  switch (k) {
    case 'Boolean':
      return { kind: 'Boolean', value: Boolean(raw) };
    case 'Int32':
      return { kind: 'Int32', value: Number(raw) | 0 };
    case 'Int64':
      return { kind: 'Int64', value: Math.floor(Number(raw)) };
    case 'Float32':
      return { kind: 'Float32', value: Number(raw) };
    case 'Float64':
      return { kind: 'Float64', value: Number(raw) };
    case 'String':
      return { kind: 'String', value: String(raw) };
    case 'Date':
      return { kind: 'Date', value: raw != null ? String(raw) : '' };
    case 'Categorical':
      return { kind: 'Categorical', value: raw != null ? String(raw) : '' };
    case 'Array':
      return {
        kind: 'Array',
        value: Array.isArray(raw)
          ? raw.map((x) => dataValueFromRaw(x, dataType.inner ?? { kind: 'Any' }))
          : [],
      };
    case 'Object':
      return {
        kind: 'Object',
        value:
          raw && typeof raw === 'object' && !Array.isArray(raw)
            ? (raw as Record<string, unknown>)
            : {},
      };
    case 'DataFrame':
      return { kind: 'DataFrame', value: String(raw ?? '') };
    case 'DataSeries':
      return { kind: 'Null' };
    default:
      return { kind: 'Null' };
  }
}
