/**
 * DataValue DTO 转换
 *
 * 前后端 DataValue 格式互转
 */

import type { DataValue } from '../domain/dataValue';
import type { DataTypeBackendFormat } from './dataType';
import { dataTypeFromBackend, dataTypeToBackend } from './dataType';

/** 后端 DataSeries 值（id 或带元数据的 struct） */
export type DataSeriesValueBackend =
  | string
  | {
      id: string;
      elementType?: DataTypeBackendFormat;
      dummyInfo?: unknown;
      timeSeriesState?: unknown;
    };

/** 后端 DataValue 序列化格式（Rust serde 外部标签枚举） */
export type DataValueBackend =
  | { Boolean: boolean }
  | { Int64: number }
  | { Float64: number }
  | { String: string }
  | { Array: DataValueBackend[] }
  | { Object: Record<string, unknown> }
  | { DataFrame: string }
  | { DataSeries: DataSeriesValueBackend }
  | { Struct: { typeKey: string; handleId: string } }
  | 'Null'
  | { Null: null };

const DATA_TYPE_LEAVES = new Set([
  'Boolean', 'Int64', 'Float64', 'String', 'Date', 'Datetime', 'Time',
  'Categorical', 'Object', 'DataFrame', 'Any',
]);

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: Record<string, unknown>, keys: string[]): boolean {
  const actual = Object.keys(value);
  return actual.length === keys.length && keys.every((key) => key in value);
}

function isRustDataTypeWire(value: unknown): boolean {
  if (!isRecord(value) || typeof value.kind !== 'string') return false;
  if (DATA_TYPE_LEAVES.has(value.kind)) return hasExactKeys(value, ['kind']);
  if (value.kind === 'Array' || value.kind === 'DataSeries') {
    return hasExactKeys(value, ['kind', 'inner']) && isRustDataTypeWire(value.inner);
  }
  if (value.kind === 'Struct') {
    return hasExactKeys(value, ['kind', 'inner']) && typeof value.inner === 'string';
  }
  return value.kind === 'OneOf'
    && hasExactKeys(value, ['kind', 'inner'])
    && Array.isArray(value.inner)
    && value.inner.every(isRustDataTypeWire);
}

function isDataSeriesWire(value: unknown): boolean {
  if (typeof value === 'string') return true;
  if (!isRecord(value) || typeof value.id !== 'string') return false;
  const allowed = new Set(['id', 'elementType', 'dummyInfo', 'timeSeriesState']);
  if (Object.keys(value).some((key) => !allowed.has(key))) return false;
  if ('elementType' in value && !isRustDataTypeWire(value.elementType)) return false;
  if ('dummyInfo' in value) {
    const info = value.dummyInfo;
    if (!isRecord(info)
      || !hasExactKeys(info, ['dropCategory', 'role'])
      || (info.dropCategory !== null && typeof info.dropCategory !== 'string')
      || !['general', 'individual', 'time'].includes(String(info.role))) return false;
  }
  return !('timeSeriesState' in value)
    || value.timeSeriesState === 'aligned'
    || value.timeSeriesState === 'unaligned';
}

/** Strict validator for Rust's externally tagged `DataValue` serde wire. */
export function isRustDataValueWire(value: unknown): value is DataValueBackend {
  if (value === 'Null') return true;
  if (!isRecord(value) || Object.keys(value).length !== 1) return false;
  if ('Boolean' in value) return typeof value.Boolean === 'boolean';
  if ('Int64' in value) return Number.isSafeInteger(value.Int64);
  if ('Float64' in value) return typeof value.Float64 === 'number' && Number.isFinite(value.Float64);
  if ('String' in value) return typeof value.String === 'string';
  if ('Array' in value) return Array.isArray(value.Array) && value.Array.every(isRustDataValueWire);
  if ('Object' in value) {
    return isRecord(value.Object) && Object.values(value.Object).every(isRustDataValueWire);
  }
  if ('DataFrame' in value) return typeof value.DataFrame === 'string';
  if ('DataSeries' in value) return isDataSeriesWire(value.DataSeries);
  if ('Struct' in value) {
    return isRecord(value.Struct)
      && hasExactKeys(value.Struct, ['typeKey', 'handleId'])
      && typeof value.Struct.typeKey === 'string'
      && typeof value.Struct.handleId === 'string';
  }
  return false;
}

/** 从后端格式解析为 DataValue */
export function dataValueFromBackend(
  v: DataValueBackend | DataValue | null | undefined
): DataValue {
  if (v == null || v === 'Null') return { kind: 'Null' };
  if (typeof v !== 'object') return { kind: 'Null' };

  if ('kind' in v && 'value' in v) return v as DataValue;
  if ('kind' in v && v.kind === 'Null') return { kind: 'Null' };

  if ('Boolean' in v) return { kind: 'Boolean', value: v.Boolean };
  if ('Int64' in v) return { kind: 'Int64', value: v.Int64 };
  if ('Float64' in v) return { kind: 'Float64', value: v.Float64 };
  if ('String' in v) return { kind: 'String', value: v.String };
  if ('Array' in v)
    return {
      kind: 'Array',
      value: (v.Array as DataValueBackend[]).map(dataValueFromBackend),
    };
  if ('Object' in v) return { kind: 'Object', value: v.Object ?? {} };
  if ('DataFrame' in v) return { kind: 'DataFrame', value: v.DataFrame };
  if ('DataSeries' in v) {
    const payload = v.DataSeries;
    if (typeof payload === 'string') {
      return { kind: 'DataSeries', value: payload };
    }
    return {
      kind: 'DataSeries',
      value: {
        id: payload.id,
        ...(payload.elementType
          ? { elementType: dataTypeFromBackend(payload.elementType) }
          : {}),
      },
    };
  }
  if ('Struct' in v) return { kind: 'Struct', value: v.Struct };
  if ('Null' in v) return { kind: 'Null' };

  return { kind: 'Null' };
}

/** 转为后端期望的格式 */
export function dataValueToBackend(
  dv: DataValue
): DataValueBackend | { Null: null } {
  switch (dv.kind) {
    case 'Boolean':
      return { Boolean: dv.value };
    case 'Int64':
      return { Int64: dv.value };
    case 'Float64':
      return { Float64: dv.value };
    case 'String':
      return { String: dv.value };
    // 后端 DataValue 无 Date/Datetime/Time/Categorical 变体，统一以 String 承载
    case 'Date':
    case 'Datetime':
    case 'Time':
    case 'Categorical':
      return { String: dv.value };
    case 'Array':
      return { Array: dv.value.map(dataValueToBackend) };
    case 'Object':
      return { Object: dv.value };
    case 'DataFrame':
      return { DataFrame: dv.value };
    case 'DataSeries': {
      if (typeof dv.value === 'string') {
        return { DataSeries: dv.value };
      }
      const payload: DataSeriesValueBackend = { id: dv.value.id };
      if (dv.value.elementType) {
        payload.elementType = dataTypeToBackend(dv.value.elementType);
      }
      return { DataSeries: payload };
    }
    case 'Struct':
      return { Struct: dv.value };
    case 'Null':
      return { Null: null };
  }
}
