/**
 * DataType DTO 转换
 *
 * 前后端 DataType 格式互转
 * 后端 Rust DataType 使用 #[serde(tag = "kind", content = "inner")]
 */

import type { DataType } from '../domain/dataType';
import { dataTypeFromKey } from '../domain/dataType';

/** 后端 DataType 格式（Rust serde: { kind: "Boolean" } 或 { kind: "Array"/"DataSeries", inner: ... }） */
export type DataTypeBackendFormat = DataType;

const DATA_TYPE_LEAVES = new Set<DataType['kind']>([
  'Boolean', 'Int64', 'Float64', 'String', 'Date', 'Datetime', 'Time', 'Categorical',
  'Object', 'Any', 'DataFrame',
]);

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  return Object.keys(value).length === keys.length
    && keys.every((key) => Object.prototype.hasOwnProperty.call(value, key));
}

export function isDataTypeBackendFormat(value: unknown): value is DataTypeBackendFormat {
  if (!isRecord(value) || typeof value.kind !== 'string') return false;
  if (DATA_TYPE_LEAVES.has(value.kind as DataType['kind'])) {
    return hasExactKeys(value, ['kind']);
  }
  if (!hasExactKeys(value, ['kind', 'inner'])) return false;
  if (value.kind === 'Struct') {
    return typeof value.inner === 'string' && value.inner.trim().length > 0;
  }
  if (value.kind === 'Array' || value.kind === 'DataSeries') {
    return isDataTypeBackendFormat(value.inner);
  }
  return value.kind === 'OneOf'
    && Array.isArray(value.inner)
    && value.inner.length > 0
    && value.inner.every(isDataTypeBackendFormat);
}

/** 转为后端期望的格式 */
export function dataTypeToBackend(dt: DataType): DataTypeBackendFormat {
  if (dt.kind === 'Array') {
    return { kind: 'Array', inner: dataTypeToBackend(dt.inner) };
  }
  if (dt.kind === 'DataSeries') {
    return { kind: 'DataSeries', inner: dataTypeToBackend(dt.inner) };
  }
  if (dt.kind === 'OneOf') {
    return { kind: 'OneOf', inner: dt.inner.map(dataTypeToBackend) };
  }
  if (dt.kind === 'Struct') {
    return { kind: 'Struct', inner: dt.inner };
  }
  return { kind: dt.kind };
}

/** 从后端格式解析为 DataType */
export function dataTypeFromBackend(
  v:
    | string
    | DataTypeBackendFormat
    | { kind?: string; inner?: unknown }
): DataType {
  if (typeof v === 'string') {
    return dataTypeFromKey(v);
  }
  if (v && typeof v === 'object') {
    const kind = (v as { kind?: string }).kind;
    const inner = (v as { inner?: unknown }).inner;
    if (kind === 'Array' && inner) {
      return { kind: 'Array', inner: dataTypeFromBackend(inner as DataTypeBackendFormat) };
    }
    if (kind === 'DataSeries' && inner) {
      return { kind: 'DataSeries', inner: dataTypeFromBackend(inner as DataTypeBackendFormat) };
    }
    if (kind === 'OneOf' && Array.isArray(inner)) {
      return { kind: 'OneOf', inner: (inner as DataTypeBackendFormat[]).map(dataTypeFromBackend) };
    }
    if (kind === 'Struct' && typeof inner === 'string') {
      return { kind: 'Struct', inner };
    }
    if (kind && kind !== 'Array' && kind !== 'DataSeries' && kind !== 'OneOf') {
      return dataTypeFromKey(kind);
    }
  }
  return { kind: 'Any' };
}
