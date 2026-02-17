/**
 * DataType DTO 转换
 *
 * 前后端 DataType 格式互转
 */

import type { DataType } from '../domain/dataType';
import { dataTypeFromKey } from '../domain/dataType';

/** 后端 DataType 格式（Rust serde：unit 为字符串，Array 为 { Array: inner }） */
export type DataTypeBackendFormat = string | { Array: DataTypeBackendFormat };

/** 转为后端期望的格式 */
export function dataTypeToBackend(dt: DataType): DataTypeBackendFormat {
  if (dt.kind === 'Array') {
    return { Array: dataTypeToBackend(dt.inner) };
  }
  return dt.kind;
}

/** 从后端格式解析为 DataType */
export function dataTypeFromBackend(
  v: string | { Array?: DataTypeBackendFormat }
): DataType {
  if (typeof v === 'string') {
    return dataTypeFromKey(v);
  }
  if (v && typeof v === 'object' && 'Array' in v && v.Array) {
    return { kind: 'Array', inner: dataTypeFromBackend(v.Array) };
  }
  return { kind: 'Any' };
}
