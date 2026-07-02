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
  | { kind: 'Int64'; value: number }
  | { kind: 'Float64'; value: number }
  | { kind: 'String'; value: string }
  | { kind: 'Date'; value: string }
  | { kind: 'Datetime'; value: string }
  | { kind: 'Time'; value: string }
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
    case 'Int64':
    case 'Float64':
    case 'String':
    case 'Date':
    case 'Datetime':
    case 'Time':
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

/** 从 JSON 原始值推断 DataValue（用于 Array<Any> 等无法静态确定元素类型的场景） */
export function inferDataValueFromJson(raw: unknown): DataValue {
  if (raw === null || raw === undefined) return { kind: 'Null' };
  if (typeof raw === 'boolean') return { kind: 'Boolean', value: raw };
  if (typeof raw === 'number') {
    return Number.isInteger(raw)
      ? { kind: 'Int64', value: raw }
      : { kind: 'Float64', value: raw };
  }
  if (typeof raw === 'string') return { kind: 'String', value: raw };
  if (Array.isArray(raw)) {
    return { kind: 'Array', value: raw.map(inferDataValueFromJson) };
  }
  if (typeof raw === 'object') {
    return { kind: 'Object', value: raw as Record<string, unknown> };
  }
  return { kind: 'Null' };
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
    case 'Int64':
      return { kind: 'Int64', value: Math.floor(Number(raw)) };
    case 'Float64':
      return { kind: 'Float64', value: Number(raw) };
    case 'String':
      return { kind: 'String', value: String(raw) };
    case 'Date':
      return { kind: 'Date', value: raw != null ? String(raw) : '' };
    case 'Datetime':
      return { kind: 'Datetime', value: raw != null ? String(raw) : '' };
    case 'Time':
      return { kind: 'Time', value: raw != null ? String(raw) : '' };
    case 'Categorical':
      return { kind: 'Categorical', value: raw != null ? String(raw) : '' };
    case 'Array':
      return {
        kind: 'Array',
        value: Array.isArray(raw)
          ? raw.map((x) =>
              dataType.inner?.kind === 'Any' || !dataType.inner
                ? inferDataValueFromJson(x)
                : dataValueFromRaw(x, dataType.inner),
            )
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
      if (typeof raw === 'string') {
        return raw.trim() ? { kind: 'DataSeries', value: raw } : { kind: 'Null' };
      }
      if (raw && typeof raw === 'object' && 'id' in raw) {
        const payload = raw as { id?: unknown; elementType?: unknown };
        const id = typeof payload.id === 'string' ? payload.id : '';
        if (!id.trim()) return { kind: 'Null' };
        return {
          kind: 'DataSeries',
          value: {
            id,
            ...(payload.elementType
              ? { elementType: payload.elementType as import('./dataType').DataType }
              : {}),
          },
        };
      }
      return { kind: 'Null' };
    case 'Any':
      return inferDataValueFromJson(raw);
    default:
      return { kind: 'Null' };
  }
}
