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
  | { kind: 'Array'; inner: DataType }
  | { kind: 'DataSeries'; inner: DataType }
  | { kind: 'Struct'; inner: string }
  | { kind: 'OneOf'; inner: DataType[] };

/** 获取 DataType 的 kind 字符串 */
export function dataTypeKind(dt: DataType): string {
  return dt.kind;
}

/** 将 DataType 转为显示字符串，如 "Int32"、"Array<String>"、"DataSeries<Float64>" */
export function dataTypeDisplay(dt: DataType): string {
  if (dt.kind === 'Array') {
    return `Array<${dataTypeDisplay(dt.inner)}>`;
  }
  if (dt.kind === 'DataSeries') {
    return `DataSeries<${dataTypeDisplay(dt.inner)}>`;
  }
  if (dt.kind === 'Struct') {
    return `Struct<${dt.inner}>`;
  }
  if (dt.kind === 'OneOf') {
    return dt.inner.map(dataTypeDisplay).join(' | ');
  }
  return dt.kind;
}

/** 从 kind 字符串创建 DataType */
export function dataTypeFromKey(key: string, inner?: DataType | string): DataType {
  const k = key as DataType['kind'];
  if (k === 'Array') {
    return { kind: 'Array', inner: (inner as DataType) ?? { kind: 'Any' } };
  }
  if (k === 'DataSeries') {
    return { kind: 'DataSeries', inner: (inner as DataType) ?? { kind: 'Any' } };
  }
  if (k === 'Struct') {
    return { kind: 'Struct', inner: (inner as string) ?? '' };
  }
  if (k === 'OneOf') {
    return { kind: 'OneOf', inner: Array.isArray(inner) ? inner : [] };
  }
  return { kind: k };
}

/** 检查数据类型是否为基础类型 */
export function isPrimitiveType(dataType: DataType): boolean {
  if (dataType.kind === 'OneOf') {
    return dataType.inner.length > 0 && dataType.inner.every(isPrimitiveType);
  }
  return ['Boolean', 'Int32', 'Int64', 'Float32', 'Float64', 'String'].includes(
    dataType.kind
  );
}

/** 检查数据类型是否为复杂类型 */
export function isComplexType(dataType: DataType): boolean {
  if (dataType.kind === 'OneOf') {
    return dataType.inner.length > 0 && dataType.inner.every(isComplexType);
  }
  return ['DataFrame', 'DataSeries', 'Object', 'Array'].includes(dataType.kind);
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

/** 从 Pin 的 type 字符串（如 "string"、"int"）映射为 DataType */
export function dataTypeFromPinType(pinType: string): DataType {
  const t = pinType?.toLowerCase() ?? 'any';
  switch (t) {
    case 'bool':
    case 'boolean':
      return { kind: 'Boolean' };
    case 'int':
    case 'int32':
      return { kind: 'Int32' };
    case 'int64':
      return { kind: 'Int64' };
    case 'float':
    case 'float32':
      return { kind: 'Float32' };
    case 'number':
    case 'float64':
      return { kind: 'Float64' };
    case 'string':
      return { kind: 'String' };
    case 'object':
      return { kind: 'Object' };
    case 'dataframe':
      return { kind: 'DataFrame' };
    case 'array':
      return { kind: 'Array', inner: { kind: 'Any' } };
    case 'dataseries':
      return { kind: 'DataSeries', inner: { kind: 'Any' } };
    case 'oneof':
    case 'any':
      return { kind: 'Any' };
    default:
      return { kind: 'String' };
  }
}
