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
  | { kind: 'DataSeries'; inner: DataTypeBackendFormat };

/** 转为后端期望的格式 */
export function dataTypeToBackend(dt: DataType): DataTypeBackendFormat {
  if (dt.kind === 'Array') {
    return { kind: 'Array', inner: dataTypeToBackend(dt.inner) };
  }
  if (dt.kind === 'DataSeries') {
    return { kind: 'DataSeries', inner: dataTypeToBackend(dt.inner) };
  }
  return { kind: dt.kind };
}

/** 从后端格式解析为 DataType */
export function dataTypeFromBackend(
  v: string | { kind?: string; inner?: DataTypeBackendFormat; Array?: DataTypeBackendFormat }
): DataType {
  if (typeof v === 'string') {
    return dataTypeFromKey(v);
  }
  if (v && typeof v === 'object') {
    if (v.kind === 'Array' && v.inner) {
      return { kind: 'Array', inner: dataTypeFromBackend(v.inner) };
    }
    if (v.kind === 'DataSeries' && v.inner) {
      return { kind: 'DataSeries', inner: dataTypeFromBackend(v.inner) };
    }
    if (v.kind && v.kind !== 'Array' && v.kind !== 'DataSeries') {
      return dataTypeFromKey(v.kind);
    }
    if ('Array' in v && v.Array) {
      return { kind: 'Array', inner: dataTypeFromBackend(v.Array) };
    }
  }
  return { kind: 'Any' };
}
