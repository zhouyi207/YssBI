/**
 * Domain Types - DataType
 *
 * 数据类型定义及领域内辅助函数
 */

/**
 * 数据类型（可辨识联合）
 */
export type DataType =
  | { kind: 'Boolean' }
  | { kind: 'Int32' }
  | { kind: 'Int64' }
  | { kind: 'Float32' }
  | { kind: 'Float64' }
  | { kind: 'String' }
  | { kind: 'Object' }
  | { kind: 'Any' }
  | { kind: 'DataFrame' }
  | { kind: 'Array'; inner: DataType };

/** 获取 DataType 的 kind 字符串 */
export function dataTypeKind(dt: DataType): string {
  return dt.kind;
}

/** 将 DataType 转为显示字符串，如 "Int32"、"Array<String>" */
export function dataTypeDisplay(dt: DataType): string {
  if (dt.kind === 'Array') {
    return `Array<${dataTypeDisplay(dt.inner)}>`;
  }
  return dt.kind;
}

/** 从 kind 字符串创建 DataType */
export function dataTypeFromKey(key: string, inner?: DataType): DataType {
  const k = key as DataType['kind'];
  if (k === 'Array') {
    return { kind: 'Array', inner: inner ?? { kind: 'Any' } };
  }
  return { kind: k };
}

/** 检查数据类型是否为基础类型 */
export function isPrimitiveType(dataType: DataType): boolean {
  return ['Boolean', 'Int32', 'Int64', 'Float32', 'Float64', 'String'].includes(
    dataType.kind
  );
}

/** 检查数据类型是否为复杂类型 */
export function isComplexType(dataType: DataType): boolean {
  return ['DataFrame', 'Object', 'Array'].includes(dataType.kind);
}

/** 获取数据类型的默认值 */
export function getDefaultValue(dataType: DataType): unknown {
  switch (dataType.kind) {
    case 'Boolean':
      return false;
    case 'Int32':
    case 'Int64':
      return 0;
    case 'Float32':
    case 'Float64':
      return 0.0;
    case 'String':
      return '';
    case 'Array':
      return [];
    case 'Object':
      return {};
    default:
      return undefined;
  }
}

/** 检查 DataType 是否与字符串类型匹配 */
export function dataTypeMatches(dt: DataType, typeStr: string): boolean {
  if (typeStr === 'any') return true;
  return dataTypeDisplay(dt) === typeStr || dataTypeKind(dt) === typeStr;
}
