/**
 * DataType DTO 转换
 *
 * 前后端 DataType 格式互转
 * 后端 Rust DataType 使用 #[serde(tag = "kind", content = "inner")]
 */

import type { DataType } from '../domain/dataType';
import { dataTypeFromKey } from '../domain/dataType';

/** 后端 DataType 格式（Rust serde: { kind: "Boolean" } 或 { kind: "Array"/"DataSeries", inner: ... }） */
export type DataTypeBackendFormat =
  | { kind: string }
  | { kind: 'Array'; inner: DataTypeBackendFormat }
  | { kind: 'DataSeries'; inner: DataTypeBackendFormat }
  | { kind: 'Struct'; inner: string }
  | { kind: 'OneOf'; inner: DataTypeBackendFormat[] };

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
