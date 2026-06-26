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
  | { kind: 'Date' }
  | { kind: 'Categorical' }
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

/** 在顶层（不在 `<>` 内部）按分隔符拆分字符串，对齐 Rust split_top_level */
function splitTopLevel(s: string, sep: string): string[] {
  const parts: string[] = [];
  let depth = 0;
  let start = 0;
  for (let i = 0; i < s.length; i++) {
    const c = s[i];
    if (c === '<') depth++;
    else if (c === '>') depth = Math.max(0, depth - 1);
    else if (c === sep && depth === 0) {
      const part = s.slice(start, i).trim();
      if (part) parts.push(part);
      start = i + 1;
    }
  }
  const tail = s.slice(start).trim();
  if (tail) parts.push(tail);
  return parts;
}

/** 构造 OneOf，自动展平嵌套并去重；对齐 Rust DataType::one_of */
function oneOf(types: DataType[]): DataType {
  const flat: DataType[] = [];
  for (const t of types) {
    if (t.kind === 'Any') return { kind: 'Any' };
    const members = t.kind === 'OneOf' ? t.inner : [t];
    for (const m of members) {
      if (m.kind === 'Any') return { kind: 'Any' };
      if (!flat.some(f => dataTypeDisplay(f) === dataTypeDisplay(m))) flat.push(m);
    }
  }
  if (flat.length === 0) return { kind: 'Any' };
  if (flat.length === 1) return flat[0];
  return { kind: 'OneOf', inner: flat };
}

/**
 * 从显示字符串解析为 DataType，对齐 Rust `DataType::from_str`。
 * 支持 Date / Categorical / 裸 DataSeries / 嵌套容器 / `|` 联合类型。
 * 解析失败返回 null（任一成员失败则整体失败，与 Rust collect 行为一致）。
 */
export function dataTypeFromDisplayString(s: string): DataType | null {
  const trimmed = s.trim();

  const parts = splitTopLevel(trimmed, '|');
  if (parts.length > 1) {
    const types: DataType[] = [];
    for (const p of parts) {
      const t = dataTypeFromDisplayString(p);
      if (t === null) return null;
      types.push(t);
    }
    return oneOf(types);
  }

  switch (trimmed) {
    case 'Boolean': return { kind: 'Boolean' };
    case 'Int32': return { kind: 'Int32' };
    case 'Int64': return { kind: 'Int64' };
    case 'Float32': return { kind: 'Float32' };
    case 'Float64': return { kind: 'Float64' };
    case 'String': return { kind: 'String' };
    case 'Date': return { kind: 'Date' };
    case 'Categorical': return { kind: 'Categorical' };
    case 'Object': return { kind: 'Object' };
    case 'DataFrame': return { kind: 'DataFrame' };
    case 'DataSeries': return { kind: 'DataSeries', inner: { kind: 'Any' } };
    case 'Any': return { kind: 'Any' };
  }

  const arrayMatch = trimmed.match(/^Array<(.+)>$/);
  if (arrayMatch) {
    const inner = dataTypeFromDisplayString(arrayMatch[1]);
    return inner ? { kind: 'Array', inner } : null;
  }
  const dsMatch = trimmed.match(/^DataSeries<(.+)>$/);
  if (dsMatch) {
    const inner = dataTypeFromDisplayString(dsMatch[1]);
    return inner ? { kind: 'DataSeries', inner } : null;
  }
  const structMatch = trimmed.match(/^Struct<(.+)>$/);
  if (structMatch) {
    return { kind: 'Struct', inner: structMatch[1] };
  }

  return null;
}

/** 从 kind 字符串创建 DataType（支持后端返回的 type 如 "Date"） */
export function dataTypeFromKey(key: string, inner?: DataType | string): DataType {
  const k = (key?.trim() || 'Any') as DataType['kind'];
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
  return ['Boolean', 'Int32', 'Int64', 'Float32', 'Float64', 'String', 'Date', 'Categorical'].includes(
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
    case 'Date':
    case 'Categorical':
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
    case 'date':
      return { kind: 'Date' };
    case 'categorical':
    case 'category':
      return { kind: 'Categorical' };
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
