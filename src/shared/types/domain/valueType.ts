import type { DeepReadonly } from "@/shared/types/deepReadonly";
import { SEMANTIC_TYPES, type SemanticType } from "./database";

/** Analysis meaning and structure; physical storage stays in field/value metadata. */
export type ValueType =
  | { kind: "Scalar"; inner: SemanticType }
  | { kind: "Object" }
  | { kind: "Any" }
  | { kind: "DataFrame" }
  | { kind: "Array"; inner: ValueType }
  | { kind: "DataSeries"; inner: ValueType }
  | { kind: "Struct"; inner: string }
  | { kind: "OneOf"; inner: ValueType[] };

export function isBackendDataType(value: unknown): value is ValueType {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const record = value as Record<string, unknown>;
  if (["Object", "Any", "DataFrame"].includes(record.kind as string))
    return Object.keys(record).length === 1;
  if (Object.keys(record).length !== 2 || !("inner" in record)) return false;
  if (record.kind === "Scalar") return SEMANTIC_TYPES.includes(record.inner as SemanticType);
  if (record.kind === "Struct")
    return typeof record.inner === "string" && record.inner.trim() !== "";
  if (record.kind === "Array" || record.kind === "DataSeries")
    return isBackendDataType(record.inner);
  return (
    record.kind === "OneOf" &&
    Array.isArray(record.inner) &&
    record.inner.length > 0 &&
    record.inner.every(isBackendDataType)
  );
}

export const CONSTANT_SELECTABLE_DATA_TYPE_KINDS = [
  ...SEMANTIC_TYPES,
  "DataFrame",
  "DataSeries",
] as const;
export const DATA_SERIES_ELEMENT_TYPE_KINDS = SEMANTIC_TYPES;
export const DEFAULT_ARRAY_VALUE: readonly number[] = [];
export const DEFAULT_OBJECT_VALUE: Readonly<Record<string, number>> = {};

export function dataTypeKind(type: ValueType): string {
  return type.kind === "Scalar" ? type.inner : type.kind;
}

export function dataTypeDisplay(type: DeepReadonly<ValueType>): string {
  switch (type.kind) {
    case "Scalar":
      return type.inner;
    case "Array":
    case "DataSeries":
      return `${type.kind}<${dataTypeDisplay(type.inner)}>`;
    case "Struct":
      return `Struct<${type.inner}>`;
    case "OneOf":
      return type.inner.map(dataTypeDisplay).join(" | ");
    default:
      return type.kind;
  }
}

function splitTopLevel(source: string): string[] | null {
  let depth = 0;
  let start = 0;
  const parts: string[] = [];
  for (let i = 0; i < source.length; i++) {
    if (source[i] === "<") depth++;
    else if (source[i] === ">" && --depth < 0) return null;
    else if (source[i] === "|" && depth === 0) {
      if (!source.slice(start, i).trim()) return null;
      parts.push(source.slice(start, i).trim());
      start = i + 1;
    }
  }
  if (depth || !source.slice(start).trim()) return null;
  parts.push(source.slice(start).trim());
  return parts;
}

function oneOf(types: ValueType[]): ValueType {
  const flat: ValueType[] = [];
  for (const type of types) {
    if (type.kind === "Any") return type;
    for (const member of type.kind === "OneOf" ? type.inner : [type]) {
      if (member.kind === "Any") return member;
      if (!flat.some((item) => dataTypeDisplay(item) === dataTypeDisplay(member)))
        flat.push(member);
    }
  }
  return flat.length === 0
    ? { kind: "Any" }
    : flat.length === 1
      ? flat[0]!
      : { kind: "OneOf", inner: flat };
}

export function dataTypeFromDisplayString(source: string): ValueType | null {
  const trimmed = source.trim();
  const parts = splitTopLevel(trimmed);
  if (!parts) return null;
  if (parts.length > 1) {
    const types = parts.map(dataTypeFromDisplayString);
    return types.every((type): type is ValueType => type !== null) ? oneOf(types) : null;
  }
  if (SEMANTIC_TYPES.includes(trimmed as SemanticType))
    return { kind: "Scalar", inner: trimmed as SemanticType };
  if (trimmed === "Any" || trimmed === "Object" || trimmed === "DataFrame")
    return { kind: trimmed };
  if (trimmed === "DataSeries") return { kind: "DataSeries", inner: { kind: "Any" } };
  const series = trimmed.match(/^(Array|DataSeries)<(.+)>$/);
  if (series) {
    const inner = dataTypeFromDisplayString(series[2]!);
    return inner ? { kind: series[1] as "Array" | "DataSeries", inner } : null;
  }
  const named = trimmed.match(/^Struct<([^<>]+)>$/);
  return named ? { kind: "Struct", inner: named[1]! } : null;
}

export function dataTypeFromKey(key: string, inner?: ValueType | string): ValueType {
  if (SEMANTIC_TYPES.includes(key as SemanticType))
    return { kind: "Scalar", inner: key as SemanticType };
  if (key === "Array" || key === "DataSeries")
    return {
      kind: key,
      inner: typeof inner === "object" ? inner : { kind: "Scalar", inner: "Numeric" },
    };
  if (key === "Struct") return { kind: "Struct", inner: typeof inner === "string" ? inner : "" };
  return dataTypeFromDisplayString(key) ?? { kind: "Any" };
}

export function isPrimitiveType(type: ValueType): boolean {
  return (
    type.kind === "Scalar" ||
    (type.kind === "OneOf" && type.inner.length > 0 && type.inner.every(isPrimitiveType))
  );
}

export function isComplexType(type: ValueType): boolean {
  return (
    ["DataFrame", "DataSeries", "Object", "Array"].includes(type.kind) ||
    (type.kind === "OneOf" && type.inner.length > 0 && type.inner.every(isComplexType))
  );
}

export function getDefaultValue(type: ValueType): unknown {
  if (type.kind === "Scalar")
    return type.inner === "Numeric" ? 0 : type.inner === "Binary" ? false : "";
  if (type.kind === "Array") return [];
  if (type.kind === "Object") return {};
  return undefined;
}
