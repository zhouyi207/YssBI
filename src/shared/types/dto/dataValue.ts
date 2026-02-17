/**
 * DataValue DTO 转换
 *
 * 前后端 DataValue 格式互转
 */

import type { DataValue } from '../domain/dataValue';

/** 后端 DataValue 序列化格式（Rust serde 外部标签枚举） */
export type DataValueBackend =
  | { Boolean: boolean }
  | { Int32: number }
  | { Int64: number }
  | { Float32: number }
  | { Float64: number }
  | { String: string }
  | { Array: DataValueBackend[] }
  | { Object: Record<string, unknown> }
  | { DataFrame: string }
  | { Null: null };

/** 从后端格式解析为 DataValue */
export function dataValueFromBackend(
  v: DataValueBackend | DataValue | null | undefined
): DataValue {
  if (v == null) return { kind: 'Null' };
  if (typeof v !== 'object') return { kind: 'Null' };

  if ('kind' in v && 'value' in v) return v as DataValue;
  if ('kind' in v && v.kind === 'Null') return { kind: 'Null' };

  if ('Boolean' in v) return { kind: 'Boolean', value: v.Boolean };
  if ('Int32' in v) return { kind: 'Int32', value: v.Int32 };
  if ('Int64' in v) return { kind: 'Int64', value: v.Int64 };
  if ('Float32' in v) return { kind: 'Float32', value: v.Float32 };
  if ('Float64' in v) return { kind: 'Float64', value: v.Float64 };
  if ('String' in v) return { kind: 'String', value: v.String };
  if ('Array' in v)
    return {
      kind: 'Array',
      value: (v.Array as DataValueBackend[]).map(dataValueFromBackend),
    };
  if ('Object' in v) return { kind: 'Object', value: v.Object ?? {} };
  if ('DataFrame' in v) return { kind: 'DataFrame', value: v.DataFrame };
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
    case 'Int32':
      return { Int32: dv.value };
    case 'Int64':
      return { Int64: dv.value };
    case 'Float32':
      return { Float32: dv.value };
    case 'Float64':
      return { Float64: dv.value };
    case 'String':
      return { String: dv.value };
    case 'Array':
      return { Array: dv.value.map(dataValueToBackend) };
    case 'Object':
      return { Object: dv.value };
    case 'DataFrame':
      return { DataFrame: dv.value };
    case 'Null':
      return { Null: null };
  }
}
